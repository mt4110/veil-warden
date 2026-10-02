// SPDX-License-Identifier: MIT
use std::{path::PathBuf, time::Duration};
#[derive(Debug)]
pub struct Options {
    pub object: PathBuf,
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
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "veil-warden --object PATH [--interval-ms 10..60000] [--samples N]\nCounts egress SKBs in the dedicated VM test slice; always allows traffic."
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
    Ok(Some(Options {
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
    fn finite_run_and_help() {
        let v = parse(args("--object counter --samples 3 --interval-ms 10"))
            .unwrap()
            .unwrap();
        assert_eq!(v.samples, 3);
        assert_eq!(v.interval, Duration::from_millis(10));
        assert!(parse(args("--help")).unwrap().is_none());
    }
}
