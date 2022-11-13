# ifstat-rs

A program analogous to ifstat from iproute2 package
(https://archlinux.org/packages/core/x86_64/iproute2/). Shows network device
speed from /proc/net/dev. See man 5 proc.

Bytes per second are displayed with binary prefixes in powers of 1024, but
bits/s use decimal prefixes from SI in powers of 1000.

## Help

```
Usage: ifstat-rs --history-file <HISTORY_FILE>

Options:
  -f, --history-file <HISTORY_FILE>  Name of a history file
  -h, --help                         Print help information
  -V, --version                      Print version information
```

## Example

```bash
$ cargo run -- -f /tmp/ifstat-rs.$(id -u)
    Finished dev [unoptimized + debuginfo] target(s) in 0.08s
     Running `target/debug/ifstat-rs -f /tmp/ifstat-rs.1000`
 Interface            Receive                        Transmit
    enp3s0          0.00 B/s (0.00 bit/s)          0.00 B/s (0.00 bit/s)
        lo       534.56 B/s (4.28 Kbit/s)       534.56 B/s (4.28 Kbit/s)
     wlan0       93.55 B/s (748.38 bit/s)          0.00 B/s (0.00 bit/s)
```
