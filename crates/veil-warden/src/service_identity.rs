// SPDX-License-Identifier: MIT
//! Loaded native system services only. Never calls LoadUnit, StartUnit or BPF.
use crate::diagnose::{non_symlink_directory, read_bounded, validate_scope};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(crate) fn validate_unit(unit: &str) -> Result<(), &'static str> {
    if unit.len() > 255
        || !unit.ends_with(".service")
        || unit == ".service"
        || !unit
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.@:-".contains(&b))
        || unit.contains("@.")
        || matches!(
            unit,
            "docker.service" | "containerd.service" | "podman.service"
        )
    {
        return Err("unsupported native service / 対象は明示した通常のserviceに限定します");
    }
    validate_scope(&format!("/system.slice/{unit}"))
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub unit: String,
    pub pid: u32,
    pub cgroup: String,
    pub invocation: String,
    kind: String,
}
// busctl terse output: accept only the restricted scalar subset needed here.
fn string(line: &str, signature: &str) -> Result<String, &'static str> {
    let value = line
        .strip_prefix(&format!("{signature} \""))
        .and_then(|s| s.strip_suffix('"'))
        .ok_or("malformed-property")?;
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"/_.@:-".contains(&b))
    {
        return Err("unsupported-property-encoding");
    }
    Ok(value.to_owned())
}
fn decode(unit: &str, service: &str) -> Result<Identity, &'static str> {
    let u: Vec<_> = unit.lines().collect();
    let s: Vec<_> = service.lines().collect();
    if u.len() != 5 || s.len() != 4 {
        return Err("missing-property");
    }
    let name = string(u[0], "s")?;
    validate_unit(&name)?;
    if string(u[1], "s")? != "loaded"
        || string(u[2], "s")? != "active"
        || string(u[3], "s")? != "running"
    {
        return Err("not-active-running");
    }
    let bytes: Vec<_> = u[4]
        .strip_prefix("ay 16 ")
        .ok_or("missing-invocation")?
        .split_whitespace()
        .map(str::parse::<u8>)
        .collect::<Result<_, _>>()
        .map_err(|_| "malformed-invocation")?;
    if bytes.len() != 16 || bytes.iter().all(|b| *b == 0) {
        return Err("missing-invocation");
    }
    let invocation = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let kind = string(s[0], "s")?;
    if !matches!(kind.as_str(), "simple" | "exec" | "notify" | "forking") {
        return Err("unsupported-service-type");
    }
    let pid = s[1]
        .strip_prefix("u ")
        .ok_or("malformed-pid")?
        .parse::<u32>()
        .map_err(|_| "malformed-pid")?;
    if pid <= 1 {
        return Err("missing-main-pid");
    }
    let cgroup = string(s[2], "s")?;
    validate_scope(&cgroup)?;
    if cgroup.rsplit('/').next() != Some(name.as_str()) || s[3] != "b false" {
        return Err("delegated-or-indirect-scope");
    }
    Ok(Identity {
        unit: name,
        pid,
        cgroup,
        invocation,
        kind,
    })
}

// Fixed executable locations avoid a caller-controlled PATH executable. Every
// command has a shared deadline and capped output, including a stuck bus client.
fn bus(args: &[&str], deadline: Instant) -> Result<String, &'static str> {
    if Instant::now() >= deadline {
        return Err("bus-deadline-expired");
    }
    let exe = ["/usr/bin/busctl", "/run/current-system/sw/bin/busctl"]
        .into_iter()
        .find(|p| Path::new(p).is_file())
        .ok_or("busctl-unavailable")?;
    let mut child = Command::new(exe)
        .args(["--system", "--auto-start=no", "--timeout=1s"])
        .args(args)
        .env_remove("DBUS_SYSTEM_BUS_ADDRESS")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "bus-unavailable")?;
    let pipe = child.stdout.take().ok_or("missing-pipe")?;
    let reader = std::thread::spawn(move || {
        let mut data = Vec::new();
        let result = pipe.take(65537).read_to_end(&mut data);
        (result, data)
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("bus-timeout-or-wait-failure");
            }
        }
    };
    let (read, data) = reader.join().map_err(|_| "bus-read-failure")?;
    read.map_err(|_| "bus-read-failure")?;
    if !status?.success() || data.len() > 65536 {
        return Err("bus-query-failed");
    }
    String::from_utf8(data).map_err(|_| "bus-output-encoding")
}
fn snapshot(unit: &str, deadline: Instant) -> Result<Identity, &'static str> {
    let object = string(
        bus(
            &[
                "call",
                "org.freedesktop.systemd1",
                "/org/freedesktop/systemd1",
                "org.freedesktop.systemd1.Manager",
                "GetUnit",
                "s",
                unit,
            ],
            deadline,
        )?
        .trim(),
        "o",
    )?;
    if !object.starts_with("/org/freedesktop/systemd1/unit/") {
        return Err("unexpected-object");
    }
    let u = bus(
        &[
            "get-property",
            "org.freedesktop.systemd1",
            &object,
            "org.freedesktop.systemd1.Unit",
            "Id",
            "LoadState",
            "ActiveState",
            "SubState",
            "InvocationID",
        ],
        deadline,
    )?;
    let s = bus(
        &[
            "get-property",
            "org.freedesktop.systemd1",
            &object,
            "org.freedesktop.systemd1.Service",
            "Type",
            "MainPID",
            "ControlGroup",
            "Delegate",
        ],
        deadline,
    )?;
    decode(&u, &s)
}
fn process_scope(text: &str) -> Result<&str, &'static str> {
    let scopes: Vec<_> = text.lines().filter_map(|l| l.strip_prefix("0::")).collect();
    if scopes.len() != 1 {
        return Err("missing-v2-process-scope");
    }
    Ok(scopes[0])
}
fn start_ticks(text: &str, pid: u32) -> Result<u64, &'static str> {
    if !text.starts_with(&format!("{pid} (")) {
        return Err("pid-mismatch");
    }
    text.rsplit_once(") ")
        .and_then(|(_, s)| s.split_whitespace().nth(19))
        .and_then(|s| s.parse().ok())
        .filter(|v| *v > 0)
        .ok_or("missing-start-time")
}
#[cfg(unix)]
fn fingerprint(identity: &Identity) -> Result<(u64, u64, u64, String), &'static str> {
    use std::os::unix::fs::MetadataExt;
    let proc = format!("/proc/{}", identity.pid);
    for ns in ["pid", "mnt", "cgroup"] {
        let target =
            std::fs::read_link(format!("{proc}/ns/{ns}")).map_err(|_| "namespace-unreadable")?;
        let manager = std::fs::read_link(format!("/proc/1/ns/{ns}"))
            .map_err(|_| "manager-namespace-unreadable")?;
        let own = std::fs::read_link(format!("/proc/self/ns/{ns}"))
            .map_err(|_| "namespace-unreadable")?;
        if own != target || own != manager {
            return Err("namespace-mismatch");
        }
    }
    let own = read_bounded(Path::new("/proc/self/cgroup")).ok_or("self-scope-unreadable")?;
    let own = process_scope(&own)?;
    if own == identity.cgroup || own.starts_with(&format!("{}/", identity.cgroup)) {
        return Err("self-or-parent-service");
    }
    let scope = read_bounded(Path::new(&format!("{proc}/cgroup"))).ok_or("process-unreadable")?;
    if process_scope(&scope)? != identity.cgroup {
        return Err("pid-cgroup-mismatch");
    }
    let stat = read_bounded(Path::new(&format!("{proc}/stat"))).ok_or("process-unreadable")?;
    let ticks = start_ticks(&stat, identity.pid)?;
    let mount = Path::new("/sys/fs/cgroup");
    if !non_symlink_directory(mount, &identity.cgroup)
        || read_bounded(&mount.join("cgroup.controllers")).is_none()
    {
        return Err("cgroup-unverified");
    }
    let metadata =
        std::fs::metadata(mount.join(&identity.cgroup[1..])).map_err(|_| "cgroup-unreadable")?;
    let boot =
        read_bounded(Path::new("/proc/sys/kernel/random/boot_id")).ok_or("boot-id-unreadable")?;
    Ok((ticks, metadata.dev(), metadata.ino(), boot))
}
fn unchanged(
    first: &Identity,
    second: &Identity,
    before: &(u64, u64, u64, String),
    after: &(u64, u64, u64, String),
) -> Result<(), &'static str> {
    if first != second || before != after {
        Err("identity-changed-during-read")
    } else {
        Ok(())
    }
}

pub(crate) fn collect(unit: &str) -> Result<Identity, &'static str> {
    validate_unit(unit)?;
    if !cfg!(target_os = "linux") {
        return Err("linux-systemd-required");
    }
    let deadline = Instant::now() + Duration::from_secs(8);
    let first = snapshot(unit, deadline)?;
    #[cfg(unix)]
    {
        let before = fingerprint(&first)?;
        let second = snapshot(unit, deadline)?;
        unchanged(&first, &second, &before, &fingerprint(&second)?)?;
        if Instant::now() >= deadline {
            return Err("identity-changed-during-read");
        }
        Ok(second)
    }
    #[cfg(not(unix))]
    {
        Err("unsupported-platform")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_restart_pid_reuse_cgroup_replacement_and_boot_change() {
        let first = decode(U, S).unwrap();
        let before = (123, 4, 5, "boot-a".to_owned());
        assert!(unchanged(&first, &decode(U, S).unwrap(), &before, &before).is_ok());
        for after in [
            (124, 4, 5, "boot-a".to_owned()),
            (123, 4, 6, "boot-a".to_owned()),
            (123, 4, 5, "boot-b".to_owned()),
        ] {
            assert!(unchanged(&first, &first, &before, &after).is_err());
        }
        assert!(
            unchanged(
                &first,
                &decode(&U.replace("1 2 3", "2 2 3"), S).unwrap(),
                &before,
                &before
            )
            .is_err()
        );
    }
    const U: &str = "s \"app.service\"\ns \"loaded\"\ns \"active\"\ns \"running\"\nay 16 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16\n";
    const S: &str = "s \"simple\"\nu 42\ns \"/system.slice/app.service\"\nb false\n";
    #[test]
    fn expired_deadline_never_starts_a_bus_client() {
        assert_eq!(
            bus(&[], Instant::now() - Duration::from_secs(1)),
            Err("bus-deadline-expired")
        );
    }
    #[test]
    fn strict_native_service_inputs() {
        for name in [
            "",
            "../app.service",
            "app",
            "ssh.service",
            "sshd@foo.service",
            "veil-warden.service",
            "docker.service",
            "app@.service",
            "app.service\n",
        ] {
            assert!(validate_unit(name).is_err(), "{name:?}");
        }
        assert!(validate_unit("app@foo.service").is_ok());
    }
    #[test]
    fn validates_typed_properties_and_lifecycle() {
        assert_eq!(decode(U, S).unwrap().pid, 42);
        for bad in [
            S.replace("u 42", "u 0"),
            S.replace("b false", "b true"),
            S.replace("simple", "oneshot"),
            S.replace("app.service", "other.service"),
            S.replace("u 42", "t 42"),
        ] {
            assert!(decode(U, &bad).is_err());
        }
        for bad in [
            U.replace("running", "dead"),
            U.replace("active\"", "inactive\""),
            U.replace("ay 16", "ay 15"),
            U.replace("app.service", "ssh.service"),
        ] {
            assert!(decode(&bad, S).is_err());
        }
        assert!(string("s \"a\\nb\"", "s").is_err());
    }
    #[test]
    fn process_identity_is_exact_not_prefix() {
        assert_eq!(process_scope("0::/a.service\n").unwrap(), "/a.service");
        assert!(process_scope("0::/a\n0::/b").is_err());
        let stat = format!(
            "42 (name ) with spaces) S {} 123 0",
            vec!["0"; 18].join(" ")
        );
        assert_eq!(start_ticks(&stat, 42), Ok(123));
        assert!(start_ticks(&stat, 43).is_err());
    }
}
