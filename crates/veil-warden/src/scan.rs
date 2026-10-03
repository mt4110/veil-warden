// SPDX-License-Identifier: MIT
//! Explicit one-shot argv evaluation. No payload, environ, or policy access.
use std::{
    fmt,
    io::{self, Read},
    time::{Duration, Instant},
};
const MAX_BYTES: usize = 16 * 1024;
const DEADLINE: Duration = Duration::from_secs(2);
pub const RULE_IDS: [&str; 6] = [
    "creds.aws.access_key_id",
    "creds.aws.secret_key_config",
    "creds.github.pat.ghp",
    "creds.github.pat.long",
    "creds.slack.token.legacy",
    "creds.key.private_pem",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub pid: u32,
    pub start_ticks: u64,
}
impl Target {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let (pid, ticks) = text
            .split_once(':')
            .ok_or("scan target requires PID:START_TICKS")?;
        let target = Self {
            pid: pid.parse().map_err(|_| "invalid scan PID")?,
            start_ticks: ticks.parse().map_err(|_| "invalid start ticks")?,
        };
        if target.pid == 0 || target.start_ticks == 0 {
            return Err("scan identity must be nonzero");
        }
        Ok(target)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Evaluated,
    Unavailable,
    Gone,
    IdentityChanged,
    OutOfScope,
    TooLarge,
    InvalidUtf8,
    Incomplete,
    Failed,
    Timeout,
}
impl Status {
    fn code(self) -> &'static str {
        match self {
            Self::Evaluated => "evaluated",
            Self::Unavailable => "unavailable",
            Self::Gone => "gone",
            Self::IdentityChanged => "identity_changed",
            Self::OutOfScope => "out_of_scope",
            Self::TooLarge => "too_large",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::Incomplete => "incomplete",
            Self::Failed => "failed",
            Self::Timeout => "timeout",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        [
            Self::Evaluated,
            Self::Unavailable,
            Self::Gone,
            Self::IdentityChanged,
            Self::OutOfScope,
            Self::TooLarge,
            Self::InvalidUtf8,
            Self::Incomplete,
            Self::Failed,
            Self::Timeout,
        ]
        .into_iter()
        .find(|v| v.code() == s)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub status: Status,
    pub counts: [u32; 6],
}
impl Summary {
    fn state(status: Status) -> Self {
        Self {
            status,
            counts: [0; 6],
        }
    }
    pub fn total(self) -> u32 {
        self.counts.iter().sum()
    }
}
impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "argv_scan={} count={} rules=",
            self.status.code(),
            self.total()
        )?;
        for (i, count) in self.counts.iter().enumerate().filter(|(_, n)| **n != 0) {
            write!(f, "{}:{count},", RULE_IDS[i])?;
        }
        write!(
            f,
            " warning={} transmission=unknown auto_deny=false",
            if self.status != Status::Evaluated {
                "unevaluated"
            } else if self.total() > 0 {
                "possible_secret"
            } else {
                "no_match"
            }
        )
    }
}
fn bounded(mut input: impl Read, max: usize) -> Result<Vec<u8>, Status> {
    let mut bytes = Vec::with_capacity(max + 1);
    input
        .by_ref()
        .take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => Status::Gone,
            _ => Status::Unavailable,
        })?;
    if bytes.len() > max {
        return Err(Status::TooLarge);
    }
    Ok(bytes)
}
fn start_ticks(bytes: &[u8]) -> Result<u64, Status> {
    let text = std::str::from_utf8(bytes).map_err(|_| Status::Incomplete)?;
    let (_, fields) = text.rsplit_once(')').ok_or(Status::Incomplete)?;
    // Remaining field 0 is state (stat field 3); starttime is stat field 22.
    fields
        .split_whitespace()
        .nth(19)
        .ok_or(Status::Incomplete)?
        .parse()
        .map_err(|_| Status::Incomplete)
}
fn in_scope(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    text.lines().any(|s| {
        s.strip_prefix("0::/warden.slice/warden-test.slice")
            .is_some_and(|tail| tail.is_empty() || tail.starts_with('/'))
    })
}
fn evaluate(bytes: &[u8]) -> Summary {
    if bytes.len() > MAX_BYTES {
        return Summary::state(Status::TooLarge);
    }
    if bytes.is_empty() || bytes.last() != Some(&0) {
        return Summary::state(Status::Incomplete);
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Summary::state(Status::InvalidUtf8);
    };
    let rules = veil_core::get_default_rules();
    let mut summary = Summary::state(Status::Evaluated);
    for (index, id) in RULE_IDS.iter().enumerate() {
        let Some(rule) = rules.iter().find(|r| r.id == *id && r.category == "secret") else {
            return Summary::state(Status::Failed);
        };
        // Each NUL-delimited argument is independent: never invent cross-argument matches.
        for arg in text.split('\0') {
            for matched in rule.pattern.find_iter(arg) {
                if rule
                    .validator
                    .is_none_or(|validate| validate(matched.as_str()))
                {
                    summary.counts[index] += 1;
                }
            }
        }
    }
    summary
}
#[cfg(target_os = "linux")]
fn read_target(target: Target) -> Summary {
    use std::{fs::File, os::fd::AsRawFd};
    let result = (|| {
        if std::fs::read_to_string("/etc/hostname")
            .map_err(|_| Status::Unavailable)?
            .trim()
            != "veil-warden-sandbox"
        {
            return Err(Status::OutOfScope);
        }
        // Anchor all reads to the opened proc directory; a reused numeric PID cannot replace it.
        let dir = File::open(format!("/proc/{}", target.pid)).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                Status::Gone
            } else {
                Status::Unavailable
            }
        })?;
        let read = |name: &str, limit: usize| -> Result<Vec<u8>, Status> {
            let file =
                File::open(format!("/proc/self/fd/{}/{name}", dir.as_raw_fd())).map_err(|e| {
                    if e.kind() == io::ErrorKind::NotFound || e.raw_os_error() == Some(3) {
                        Status::Gone
                    } else {
                        Status::Unavailable
                    }
                })?;
            bounded(file, limit)
        };
        let verify = || -> Result<(), Status> {
            if start_ticks(&read("stat", 4096)?)? != target.start_ticks {
                return Err(Status::IdentityChanged);
            }
            if !in_scope(&read("cgroup", 4096)?) {
                return Err(Status::OutOfScope);
            }
            Ok(())
        };
        verify()?;
        let bytes = read("cmdline", MAX_BYTES)?;
        verify()?;
        let summary = evaluate(&bytes);
        verify()?;
        Ok(summary)
    })();
    result.unwrap_or_else(Summary::state)
}
#[cfg(target_os = "linux")]
fn limits() -> io::Result<()> {
    use rustix::process::{self, DumpableBehavior, Resource, Rlimit, Signal};
    // Applied before reading cmdline; failures do not fall back to an unbounded scan.
    process::set_dumpable_behavior(DumpableBehavior::NotDumpable)?;
    let parent = process::getppid();
    process::set_parent_process_death_signal(Some(Signal::KILL))?;
    if parent != process::getppid() || parent.is_none_or(|p| p.as_raw_pid() == 1) {
        return Err(io::Error::other("scan parent unavailable"));
    }
    for (resource, limit) in [
        (Resource::Core, 0),
        (Resource::Cpu, 2),
        (Resource::As, 256 * 1024 * 1024),
    ] {
        process::setrlimit(
            resource,
            Rlimit {
                current: Some(limit),
                maximum: Some(limit),
            },
        )?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn child(target: Target) -> io::Result<()> {
    use std::io::Write;
    let summary = match limits() {
        Ok(()) => read_target(target),
        Err(_) => Summary::state(Status::Failed),
    };
    let mut stdout = io::stdout().lock();
    // Fixed numeric protocol. Neither errors nor input text can enter the transport.
    writeln!(
        stdout,
        "{} {} {} {} {} {} {}",
        summary.status.code(),
        summary.counts[0],
        summary.counts[1],
        summary.counts[2],
        summary.counts[3],
        summary.counts[4],
        summary.counts[5]
    )
}
fn decode(text: &str) -> Option<Summary> {
    let mut fields = text.split_whitespace();
    let status = Status::parse(fields.next()?)?;
    let mut counts = [0; 6];
    for count in &mut counts {
        *count = fields.next()?.parse().ok()?;
        if *count > MAX_BYTES as u32 {
            return None;
        }
    }
    if fields.next().is_some() || (status != Status::Evaluated && counts != [0; 6]) {
        return None;
    }
    Some(Summary { status, counts })
}
#[cfg(target_os = "linux")]
pub fn run(target: Target) -> Summary {
    use std::process::{Command, Stdio};
    let result = (|| -> Result<Summary, Status> {
        let executable = std::env::current_exe().map_err(|_| Status::Failed)?;
        let mut child = Command::new(executable)
            .arg("__scan-argv")
            .arg(format!("{}:{}", target.pid, target.start_ticks))
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Status::Failed)?;
        collect(&mut child, DEADLINE)
    })();
    result.unwrap_or_else(Summary::state)
}
#[cfg(target_os = "linux")]
fn collect(child: &mut std::process::Child, deadline: Duration) -> Result<Summary, Status> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(Status::Failed);
                }
                let output = bounded(child.stdout.take().ok_or(Status::Failed)?, 1024)?;
                return decode(std::str::from_utf8(&output).map_err(|_| Status::Failed)?)
                    .ok_or(Status::Failed);
            }
            Ok(None) if started.elapsed() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            other => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(if other.is_err() {
                    Status::Failed
                } else {
                    Status::Timeout
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_rules_redact_and_do_not_apply_source_ignore() {
        let key = format!("ghp_{}", "A".repeat(36));
        let bytes = format!("client\0{key} # veil:ignore\0");
        let result = evaluate(bytes.as_bytes());
        assert_eq!(result.status, Status::Evaluated);
        assert_eq!(result.counts[2], 1);
        let output = result.to_string();
        assert!(!output.contains(&key));
        assert!(!output.contains("client"));
        assert!(output.contains("transmission=unknown auto_deny=false"));
        assert_eq!(evaluate(b"normal\0words\0").total(), 0);
        assert_eq!(evaluate(b"ghp_short\0").total(), 0); // Known short/unknown-token miss.
        assert_eq!(
            evaluate(format!("documentation {key}\0").as_bytes()).total(),
            1
        ); // Known format-only false positive.
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "changes process-wide limits; run in an isolated VM test process"]
    fn isolated_worker_limits() {
        use rustix::process::{self, DumpableBehavior, Resource, Signal};
        limits().unwrap();
        assert_eq!(
            process::dumpable_behavior().unwrap(),
            DumpableBehavior::NotDumpable
        );
        assert_eq!(
            process::parent_process_death_signal().unwrap(),
            Some(Signal::KILL)
        );
        for (resource, limit) in [
            (Resource::Core, 0),
            (Resource::Cpu, 2),
            (Resource::As, 256 * 1024 * 1024),
        ] {
            let current = process::getrlimit(resource);
            assert_eq!(current.current, Some(limit));
            assert_eq!(current.maximum, Some(limit));
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn worker_deadline_failure_and_reaping() {
        use std::process::{Command, Stdio};
        let mut child = Command::new("sleep")
            .arg("5")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        assert_eq!(
            collect(&mut child, Duration::from_millis(20)),
            Err(Status::Timeout)
        );
        assert!(child.try_wait().unwrap().is_some());
        let mut child = Command::new("false")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        assert_eq!(collect(&mut child, DEADLINE), Err(Status::Failed));
    }
    #[test]
    fn all_pinned_secret_rules_and_no_cross_argument_match() {
        let tokens = [
            format!("AKIA{}", "A".repeat(16)),
            format!("aws_secret_access_key={}", "A".repeat(40)),
            format!("ghp_{}", "A".repeat(36)),
            format!("github_pat_{}", "A".repeat(80)),
            format!("xoxb-1234567890-1234567890-{}", "A".repeat(24)),
            "-----BEGIN PRIVATE KEY-----".into(),
        ];
        for (i, token) in tokens.iter().enumerate() {
            let result = evaluate(format!("{token}\0").as_bytes());
            assert_eq!(result.status, Status::Evaluated);
            assert_eq!(result.counts[i], 1);
        }
        assert_eq!(
            evaluate(format!("ghp_\0{}\0", "A".repeat(36)).as_bytes()).total(),
            0
        );
    }
    #[test]
    fn boundaries_never_report_clean() {
        for (bytes, status) in [
            (vec![], Status::Incomplete),
            (b"missing terminator".to_vec(), Status::Incomplete),
            (vec![255, 0], Status::InvalidUtf8),
            (vec![0; MAX_BYTES + 1], Status::TooLarge),
        ] {
            assert_eq!(evaluate(&bytes), Summary::state(status));
        }
        assert_eq!(
            bounded(io::Cursor::new(vec![0; 17]), 16),
            Err(Status::TooLarge)
        );
        struct Denied;
        impl Read for Denied {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::ErrorKind::PermissionDenied.into())
            }
        }
        assert_eq!(bounded(Denied, 16), Err(Status::Unavailable));
        assert!(decode("evaluated 0 0 0 0 0 0 secret").is_none());
        assert!(decode("failed 0 0 1 0 0 0").is_none());
        assert!(decode("evaluated 0 0 999999 0 0 0").is_none());
    }
    #[test]
    fn identity_scope_and_explicit_target() {
        assert!(Target::parse("0:1").is_err());
        assert!(Target::parse("1:0").is_err());
        assert!(Target::parse("all").is_err());
        let stat = format!("123 (a ) tricky) S {} 456 0", "0 ".repeat(18));
        assert_eq!(start_ticks(stat.as_bytes()), Ok(456));
        assert!(in_scope(
            b"0::/warden.slice/warden-test.slice/test.service\n"
        ));
        assert!(!in_scope(b"0::/warden.slice/warden-test.slice-evil/x\n"));
        assert!(!in_scope(b"0::/system.slice/sshd.service\n"));
    }
}
