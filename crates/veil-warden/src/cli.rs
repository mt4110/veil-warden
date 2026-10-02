// SPDX-License-Identifier: MIT
use std::{path::PathBuf, time::Duration};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Counter,
    Connect,
}
#[derive(Debug)]
pub struct Options {
    pub object: PathBuf,
    pub mode: Mode,
    pub duration: Duration,
    pub reader_delay: Duration,
    pub interval: Duration,
    /// Zero means run until SIGINT/SIGTERM.
    pub samples: u64,
}
pub fn parse(
    args: impl Iterator<Item = String>,
) -> Result<Option<Options>, Box<dyn std::error::Error>> {
    let mut args = args;
    let mut object = None;
    let mut interval = 1000;
    let mut samples = 0;
    let mut mode = Mode::Counter;
    let mut duration = 0;
    let mut reader_delay = 0;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "connect" if mode == Mode::Counter => mode = Mode::Connect,
            "--duration-ms" => duration = args.next().ok_or("missing duration")?.parse()?,
            "--reader-delay-ms" => {
                reader_delay = args.next().ok_or("missing reader delay")?.parse()?
            }
            "--help" | "-h" => {
                println!(
                    "veil-warden --object PATH [--interval-ms 10..60000] [--samples N]\nDefault: egress SKB counter. Add connect for TCP connect attempts (not connection success).\nconnect: [--duration-ms 0..600000] [--reader-delay-ms 0..1000 diagnostic]\nAlways allows traffic; fixed VM test slice."
                );
                return Ok(None);
            }
            "--object" => object = Some(PathBuf::from(args.next().ok_or("missing object path")?)),
            "--interval-ms" => interval = args.next().ok_or("missing interval")?.parse()?,
            "--samples" => samples = args.next().ok_or("missing samples")?.parse()?,
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    if !(10..=60000).contains(&interval) {
        return Err("interval must be 10..60000 ms".into());
    }
    if duration > 600000 || reader_delay > 1000 {
        return Err("duration/delay exceeds limit".into());
    }
    if (mode == Mode::Connect && samples != 0)
        || (mode == Mode::Counter && (duration != 0 || reader_delay != 0))
    {
        return Err("--samples is for the counter; duration/delay are for connect".into());
    }
    Ok(Some(Options {
        mode,
        duration: Duration::from_millis(duration),
        reader_delay: Duration::from_millis(reader_delay),
        object: object.ok_or("--object is required")?,
        interval: Duration::from_millis(interval),
        samples,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(s: &str) -> impl Iterator<Item = String> {
        s.split_whitespace().map(str::to_owned)
    }
    #[test]
    fn rejects_invalid_or_expanded_scope() {
        for s in [
            "",
            "--object x --interval-ms 0",
            "--object x --interval-ms 60001",
            "--object x --samples -1",
            "--object x --cgroup /",
            "--object",
        ] {
            assert!(parse(args(s)).is_err(), "{s}");
        }
    }
    #[test]
    fn connect_options_are_bounded() {
        let v = parse(args(
            "connect --object bpf --duration-ms 2000 --reader-delay-ms 500",
        ))
        .unwrap()
        .unwrap();
        assert_eq!(v.mode, Mode::Connect);
        for s in [
            "connect --object bpf --samples 1",
            "connect --object bpf --duration-ms 600001",
            "connect --object bpf --reader-delay-ms 1001",
            "--object bpf --reader-delay-ms 1",
        ] {
            assert!(parse(args(s)).is_err());
        }
    }
    #[test]
    fn finite_run_and_help() {
        let v = parse(args("--object counter --samples 3 --interval-ms 10"))
            .unwrap()
            .unwrap();
        assert_eq!(v.samples, 3);
        assert_eq!(v.interval, Duration::from_millis(10));
        assert!(parse(args("--help")).unwrap().is_none());
    }
}
