// SPDX-License-Identifier: MIT
use crate::{decode, events::attach, policy};
use aya::{
    Ebpf,
    maps::{Array, HashMap, RingBuf},
};
use std::{
    fs::{self, File},
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    os::unix::fs::MetadataExt,
    path::Path,
    time::Duration,
};
use veil_warden_common::{ACTION_ALLOWED, ACTION_DENIED, ENFORCE};

type Error = Box<dyn std::error::Error>;
const TIMEOUT: Duration = Duration::from_secs(2);
const PREFIX: &str = "/warden.slice/warden-test.slice/veil-warden-loopback-";

fn test_cgroup() -> Result<(File, u64, String), Error> {
    let membership = fs::read_to_string("/proc/self/cgroup")?;
    let scope = membership
        .trim_end()
        .strip_prefix("0::")
        .ok_or("cgroup v2 required")?;
    let token = scope
        .strip_prefix(PREFIX)
        .and_then(|s| s.strip_suffix(".scope"))
        .ok_or("run inside a veil-warden-loopback-<token>.scope under warden-test.slice")?;
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid transient scope token".into());
    }
    let path = Path::new("/sys/fs/cgroup").join(&scope[1..]);
    if fs::canonicalize(&path)? != path {
        return Err("test scope must not contain symlinks".into());
    }
    let cgroup = File::open(&path)?;
    let procs = fs::read_to_string(path.join("cgroup.procs"))?;
    let stat = fs::read_to_string(path.join("cgroup.stat"))?;
    if procs.trim() != std::process::id().to_string()
        || !stat.lines().any(|line| line == "nr_descendants 0")
    {
        return Err("test scope must contain only this process and have no child cgroups".into());
    }
    let id = cgroup.metadata()?.ino();
    Ok((cgroup, id, scope.into()))
}

fn attempt(listener: &TcpListener, stage: &str, denied: bool) -> Result<SocketAddr, Error> {
    let address = listener.local_addr()?;
    let (errno, received) = match TcpStream::connect_timeout(&address, TIMEOUT) {
        Ok(mut client) if !denied => {
            client.set_write_timeout(Some(TIMEOUT))?;
            client.write_all(b"warden")?;
            let (mut server, _) = listener.accept()?;
            server.set_nonblocking(false)?;
            server.set_read_timeout(Some(TIMEOUT))?;
            let mut payload = [0; 6];
            server.read_exact(&mut payload)?;
            if &payload != b"warden" {
                return Err("unexpected loopback payload".into());
            }
            (0, 1)
        }
        Err(error) if denied && error.raw_os_error() == Some(1) => {
            match listener.accept() {
                Err(error) if error.kind() == ErrorKind::WouldBlock => (),
                other => {
                    return Err(format!("denied connection reached listener: {other:?}").into());
                }
            }
            (1, 0)
        }
        other => return Err(format!("unexpected {stage} connect outcome: {other:?}").into()),
    };
    println!("stage={stage} destination={address} errno={errno} received={received}");
    Ok(address)
}

pub fn run(args: &[String]) -> Result<(), Error> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        println!("test-loopback --object TRUSTED_BPF_OBJECT (dedicated transient scope required)");
        return Ok(());
    }
    let [option, object] = args else {
        return Err("usage: test-loopback --object TRUSTED_BPF_OBJECT".into());
    };
    if option != "--object" {
        return Err("usage: test-loopback --object TRUSTED_BPF_OBJECT".into());
    }
    let (cgroup, id, scope) = test_cgroup()?;
    let listeners = [
        TcpListener::bind("127.0.0.1:0")?,
        TcpListener::bind("[::1]:0")?,
    ];
    for listener in &listeners {
        listener.set_nonblocking(true)?;
        attempt(listener, "before_attach", false)?;
    }
    let mut bpf = Ebpf::load_file(object)?;
    let mut events = RingBuf::try_from(bpf.take_map("EVENTS").ok_or("EVENTS missing")?)?;
    let mut mode = Array::<_, u32>::try_from(bpf.take_map("MODE").ok_or("MODE missing")?)?;
    let mut rules = HashMap::try_from(
        bpf.take_map("DENY_DESTINATIONS")
            .ok_or("DENY_DESTINATIONS missing")?,
    )?;
    let link4 = attach(&mut bpf, &cgroup, "monitor_connect4")?;
    let link6 = attach(&mut bpf, &cgroup, "monitor_connect6")?;
    println!("attached scope={scope} cgroup_id={id} links=2");
    for (stage, denied) in [
        ("observe", false),
        ("deny", true),
        ("remove", false),
        ("deny_again", true),
    ] {
        for listener in &listeners {
            let address = listener.local_addr()?;
            let key = policy::key(&address.ip().to_string(), &address.port().to_string())?;
            match stage {
                "deny" | "deny_again" => rules.insert(key, 1u32, 0)?,
                "remove" => rules.remove(&key)?,
                _ => (),
            }
            mode.set(0, if stage == "observe" { 0 } else { ENFORCE }, 0)?;
            attempt(listener, stage, denied)?;
            let raw = events.next().ok_or("missing connect event")?;
            let event =
                decode::decode(&raw).map_err(|error| format!("invalid event: {error:?}"))?;
            let expected_family = if address.is_ipv4() { 4 } else { 6 };
            if event.cgroup_id != id
                || event.tgid != std::process::id()
                || event.port != address.port()
                || event.family != expected_family
                || event.action
                    != if denied {
                        ACTION_DENIED
                    } else {
                        ACTION_ALLOWED
                    }
            {
                return Err(format!("unexpected connect event: {event:?}").into());
            }
            println!(
                "event stage={stage} tgid={} cgroup_id={} family={} port={} action={} policy_id={}",
                event.tgid,
                event.cgroup_id,
                event.family,
                event.port,
                event.action,
                event.policy_id
            );
        }
    }
    drop(link6);
    drop(link4);
    for listener in &listeners {
        attempt(listener, "after_detach", false)?;
    }
    Ok(())
}
