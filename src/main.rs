use anyhow::Context;
use std::io::{BufRead, Write};

#[derive(Copy, Clone, serde::Serialize, serde::Deserialize)]
struct DeviceStatistics {
    rx: u64,
    tx: u64,
}
impl std::ops::SubAssign for DeviceStatistics {
    fn sub_assign(&mut self, rhs: Self) {
        self.rx -= rhs.rx;
        self.tx -= rhs.tx;
    }
}
impl std::ops::Sub<DeviceStatistics> for DeviceStatistics {
    type Output = Self;
    fn sub(self, rhs: DeviceStatistics) -> Self::Output {
        let mut tmp = self;
        tmp -= rhs;
        return tmp;
    }
}

type DeviceRates = std::collections::BTreeMap<String, DeviceStatistics>;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct StatisticsDb {
    timestamp: chrono::DateTime<chrono::Utc>,
    devices: DeviceRates,
}
impl StatisticsDb {
    fn new() -> Self {
        let timestamp = chrono::Utc::now();
        let devices = DeviceRates::new();
        return Self { timestamp, devices };
    }
}

const PROC_NET_DEV_PATH: &str = "/proc/net/dev";

fn parse_proc_net_dev(hide_zero_ifs: bool) -> anyhow::Result<StatisticsDb> {
    let mut ret = StatisticsDb::new();
    let buf_reader = std::io::BufReader::new(
        std::fs::File::open(PROC_NET_DEV_PATH)
            .with_context(|| format!("Failed to open {}", PROC_NET_DEV_PATH))?,
    );
    for line in buf_reader.lines().skip(2).filter_map(|x| x.ok()) {
        let mut split = line.split_ascii_whitespace();
        let ifname = match split.next() {
            Some(x) => x.trim_end_matches(':').to_string(),
            None => return Err(anyhow::anyhow!("Missing interface name")),
        };
        let rx = match split.next() {
            Some(x) => x.parse::<u64>().context("Failed to parse rx bytes")?,
            None => return Err(anyhow::anyhow!("Missing rx bytes")),
        };
        let tx = match split.skip(7).next() {
            Some(x) => x.parse::<u64>().context("Failed to parse tx bytes")?,
            None => return Err(anyhow::anyhow!("Missing tx bytes")),
        };
        if hide_zero_ifs && rx.max(tx) == 0 {
            log::debug!("Interface '{ifname}' has zero statistics. Ignoring");
            continue;
        }
        ret.devices.insert(ifname, DeviceStatistics { rx, tx });
    }
    return Ok(ret);
}

fn subtract_device_rates(a: &DeviceRates, b: &DeviceRates) -> DeviceRates {
    let mut ret = DeviceRates::new();
    for (ifname, left_rate) in a.iter() {
        if let Some(right_rate) = b.get(ifname) {
            let result_stat = *left_rate - *right_rate;
            ret.insert(ifname.clone(), result_stat);
        } else {
            continue;
        }
    }
    return ret;
}

fn dump_stat_db(path: &str, db: &StatisticsDb) -> anyhow::Result<()> {
    let mut buf_writer = std::io::BufWriter::new(
        std::fs::File::create(path)
            .with_context(|| format!("Failed to open {}", path))?,
    );
    serde_json::to_writer(&mut buf_writer, &db).context("Serialization failed")?;
    buf_writer.flush().context("Flush failed")?;
    return Ok(());
}

fn parse_stat_db(path: &str) -> Result<StatisticsDb, Box<dyn std::error::Error>> {
    let buf_reader = std::io::BufReader::new(
        std::fs::File::open(path)
            .with_context(|| format!("Failed to open {}", path))?,
    );
    let ret = serde_json::from_reader(buf_reader).context("Failed to parse db")?;
    return Ok(ret);
}

fn is_file_exist(path: &str) -> bool {
    return std::path::Path::new(path).is_file();
}

fn get_human_value<'a>(
    value: f64,
    prefixes: &[&'a str],
    factor: f64,
) -> (f64, &'a str) {
    let mut new_value = value;
    let mut new_prefix = "";
    if value.is_finite() {
        for p in prefixes {
            if new_value <= factor {
                break;
            }
            new_value /= factor;
            new_prefix = p;
        }
    }
    return (new_value, new_prefix);
}

fn pretty_print_bytes_and_bites(value: f64, width: usize) {
    let binary_prefixes = ["Ki", "Mi", "Gi", "Ti"];
    let (pretty_binary_bytes, bytes_prefix) =
        get_human_value(value, &binary_prefixes, 1024_f64);
    let decimal_prefixes = ["K", "M", "G", "T"];
    let (pretty_decimal_bits, bits_prefix) =
        get_human_value(value * 8_f64, &decimal_prefixes, 1000_f64);
    let precision = 2;
    let combined = format!(
        "{:.precision$} {}B/s ({:.precision$} {}bit/s)",
        pretty_binary_bytes, bytes_prefix, pretty_decimal_bits, bits_prefix
    );
    print!(" {:>width$}", combined);
}

fn make_repeated_string(c: char, n: usize) -> String {
    return std::iter::repeat(c).take(n).collect();
}

fn get_sorted_ifs(db: &StatisticsDb, sort_by_stat: bool) -> Vec<String> {
    let mut v: Vec<(String, DeviceStatistics)> = db
        .devices
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if sort_by_stat {
        v.sort_by_key(|(_, v)| v.rx + v.tx);
        v.reverse();
    } else {
        v.sort_by_key(|(k, _)| k.clone());
    }
    return v.iter().map(|(k, _)| k).cloned().collect();
}

fn pretty_print_devices_speed(
    diff: &DeviceRates,
    db: &StatisticsDb,
    seconds: f64,
    hide_zero_values: bool,
    sort_by_stat: bool,
) {
    let number_width = 30;
    let ifname_width = diff.keys().map(|x| x.len()).max().unwrap_or(0).max(10);
    println!(
        "{:>ifname_width$} {:^number_width$} {:^number_width$}",
        "Interface", "Receive", "Transmit"
    );
    let sorted_ifs = get_sorted_ifs(&db, sort_by_stat);
    for ifname in sorted_ifs {
        let stat = match diff.get(&ifname) {
            Some(x) => x,
            None => continue,
        };
        print!("{:>ifname_width$}", ifname);
        for col in [stat.rx, stat.tx] {
            if hide_zero_values && col == 0 {
                print!(" {}", make_repeated_string(' ', number_width));
            } else {
                pretty_print_bytes_and_bites(col as f64 / seconds, number_width);
            }
        }
        println!();
    }
}

/// A program analogous to ifstat from iproute2 package
/// (https://archlinux.org/packages/core/x86_64/iproute2/). Shows network device speed from
/// /proc/net/dev. See man 5 proc
#[derive(Debug, clap::Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Name of a history file
    #[arg(short = 'f', long)]
    history_file: String,

    /// Hide interfaces with zero statistics
    #[arg(long)]
    hide_zero_ifs: bool,

    /// Hide zeros from fields
    #[arg(long)]
    hide_zero_values: bool,

    /// Sort devices by total statistics instead of alphabetically
    #[arg(long)]
    sort_by_stat: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    use clap::Parser;
    let args = Cli::parse();

    if is_file_exist(&args.history_file) {
        log::debug!("File `{}` exists", args.history_file);
        let a = parse_stat_db(&args.history_file)?;
        let b = parse_proc_net_dev(args.hide_zero_ifs).with_context(|| {
            format!("Failed to parse {} file", PROC_NET_DEV_PATH)
        })?;
        let diff = subtract_device_rates(&b.devices, &a.devices);
        dump_stat_db(&args.history_file, &b)
            .context("Failed to update statistics db")?;
        let interval = (b.timestamp - a.timestamp)
            .to_std()
            .context("Duration is negative!")?
            .as_secs_f64();
        log::debug!("Interval = {} s", interval);
        pretty_print_devices_speed(
            &diff,
            &b,
            interval,
            args.hide_zero_values,
            args.sort_by_stat,
        );
    } else {
        log::debug!("File `{}` does not exist", args.history_file);
        let a = parse_proc_net_dev(args.hide_zero_ifs).with_context(|| {
            format!("Failed to parse {} file", PROC_NET_DEV_PATH)
        })?;
        dump_stat_db(&args.history_file, &a)
            .context("Failed to update statistics db")?;
        pretty_print_devices_speed(
            &a.devices,
            &a,
            0_f64,
            args.hide_zero_values,
            args.sort_by_stat,
        );
    }

    return Ok(());
}
