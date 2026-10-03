// SPDX-License-Identifier: MIT
//! macOS entry point; the Linux eBPF runtime remains inside the dedicated VM.
#![forbid(unsafe_code)]
use std::{
    env,
    error::Error,
    fs,
    io::IsTerminal,
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const REPO: &str = env!("WARDEN_REPO");
const NIX: &str = env!("WARDEN_NIX");
const HELP: &str = "veil-warden [start | tui [--enforce] | demo | build | status | stop]\n  start   Start VMs in the background, build missing artifacts, run preflight\n  tui     Open the monitor (default when no command is given)\n  demo    Run the synthetic deny/remove/recovery demo\n  build   Rebuild sandbox and runtime (sandbox must be stopped)\n  status  Show SSH reachability and launchd ownership\n  stop    Stop only VMs managed by this command\nVMs stay running after the terminal closes; no login autostart.\nFirst use may download dependencies. Existing build artifacts are reused.\nCheckout moved? Re-run scripts/install-cli.sh from its new location.";
fn operation_lock() -> Result<fs::File> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = state().join("host-cli");
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("operation.lock"))?;
    file.try_lock()
        .map_err(|_| "Another veil-warden operation is running")?;
    Ok(file)
}
fn state() -> PathBuf {
    env::var_os("WARDEN_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(env::var_os("HOME").expect("HOME is required")).join(".cache")
                })
                .join("veil-warden-m0")
        })
}
fn label(role: &str) -> String {
    format!("com.mt4110.veil-warden.{role}")
}
fn domain() -> Result<String> {
    let out = Command::new("/usr/bin/id").arg("-u").output()?;
    if !out.status.success() {
        return Err("Cannot determine user ID".into());
    }
    Ok(format!("gui/{}", String::from_utf8(out.stdout)?.trim()))
}
fn loaded(role: &str) -> Result<bool> {
    Ok(Command::new("/bin/launchctl")
        .args(["print", &format!("{}/{}", domain()?, label(role))])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
}
fn reachable(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}
fn script(name: &str, args: &[String]) -> Command {
    let mut c = Command::new(NIX);
    c.args([
        "--extra-experimental-features",
        "nix-command flakes",
        "develop",
    ])
    .arg(format!("path:{REPO}"))
    .args(["--command", "bash"])
    .arg(PathBuf::from(REPO).join("scripts").join(name))
    .args(args)
    .current_dir(REPO);
    c
}
fn run(name: &str, args: &[String]) -> Result {
    if !script(name, args).status()?.success() {
        return Err(format!("{name} failed; no successful state claimed").into());
    }
    Ok(())
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn plist(role: &str, executable: &str, cache: &str, path: &str) -> String {
    let log = format!("{cache}/host-cli/{role}.log");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{}</string>
<key>ProgramArguments</key><array><string>{}</string><string>__vm</string><string>{}</string></array>
<key>EnvironmentVariables</key><dict><key>WARDEN_STATE_DIR</key><string>{}</string><key>PATH</key><string>{}</string></dict>
<key>RunAtLoad</key><true/>
<key>KeepAlive</key><false/>
<key>StandardOutPath</key><string>{}</string>
<key>StandardErrorPath</key><string>{}</string>
</dict></plist>
"#,
        xml(&label(role)),
        xml(executable),
        xml(role),
        xml(cache),
        xml(path),
        xml(&log),
        xml(&log)
    )
}
fn start_job(role: &str) -> Result {
    if loaded(role)? {
        return Ok(());
    }
    let dir = state().join("host-cli");
    fs::create_dir_all(&dir)?;
    let p = dir.join(format!("{role}.plist"));
    let cache = fs::canonicalize(state())?;
    fs::write(
        &p,
        plist(
            role,
            env::current_exe()?
                .to_str()
                .ok_or("Non-UTF8 executable path")?,
            cache.to_str().ok_or("Non-UTF8 state path")?,
            &env::var("PATH")?,
        ),
    )?;
    if !Command::new("/bin/launchctl")
        .args(["bootstrap", &domain()?])
        .arg(&p)
        .status()?
        .success()
    {
        return Err(format!("Cannot start {role}. See {}", dir.display()).into());
    }
    Ok(())
}
fn quiet_ready(role: &str) -> Result<bool> {
    let mut c = if role == "builder" {
        let mut c = Command::new("ssh");
        c.args([
            "-F",
            "/dev/null",
            "-o",
            "ConnectTimeout=3",
            "-p",
            "32222",
            "-i",
        ])
        .arg(state().join("private/operator"))
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
        ])
        .arg(format!(
            "UserKnownHostsFile={}",
            state().join("bootstrap/known_hosts").display()
        ))
        .args(["builder@127.0.0.1", "true"]);
        c
    } else {
        let mut c = Command::new("bash");
        c.arg(PathBuf::from(REPO).join("scripts/ssh-vm.sh")).args([
            "-o",
            "ConnectTimeout=3",
            "true",
        ]);
        c
    };
    Ok(c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
}
fn wait_ready(role: &str, port: u16) -> Result {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(900) {
        if reachable(port) && quiet_ready(role)? {
            return Ok(());
        }
        if start.elapsed().as_secs() % 30 == 0 {
            println!("Waiting for {role} SSH… ({}s)", start.elapsed().as_secs());
        }
        thread::sleep(Duration::from_secs(1));
    }
    Err(format!("{role} not ready after 900s. Logs: {}/host-cli/{role}.log; inspect status, then stop before retrying", state().display()).into())
}
fn ensure_builder() -> Result {
    if !reachable(32222) {
        println!("Starting build VM…");
        start_job("builder")?;
    }
    wait_ready("builder", 32222)
}
fn artifact(marker: &str, names: &[&str]) -> bool {
    fs::read_to_string(state().join(marker)).is_ok_and(|p| {
        names
            .iter()
            .all(|n| PathBuf::from(p.trim()).join(n).is_file())
    })
}
fn start() -> Result {
    let _lock = operation_lock()?;
    if !PathBuf::from(REPO).join("flake.lock").is_file() {
        return Err("Checkout missing; reinstall the CLI from its new location".into());
    }
    let bundle = artifact(
        "sandbox/build-path",
        &["kernel", "initrd", "store.img", "kernel-params"],
    );
    let runtime = artifact(
        "m1/build-path",
        &[
            "veil-warden",
            "veil-warden-ebpf",
            "veil-warden-partial-fixture",
        ],
    );
    if !bundle || !runtime {
        ensure_builder()?;
    }
    if !bundle {
        println!("Building sandbox…");
        run("build-vm.sh", &[])?;
    }
    if !runtime {
        println!("Building runtime…");
        run("build-counter-vm.sh", &[])?;
    }
    if !reachable(32223) {
        println!("Starting sandbox VM…");
        start_job("sandbox")?;
    }
    wait_ready("sandbox", 32223)?;
    let report = script("ssh-vm.sh", &["sudo warden-preflight".into()]).output()?;
    let log = state().join("host-cli/preflight.log");
    fs::write(&log, &report.stdout)?;
    if !report.status.success() {
        eprint!("{}", String::from_utf8_lossy(&report.stderr));
        return Err(format!("Sandbox preflight failed; see {}", log.display()).into());
    }
    println!("Sandbox preflight passed.");
    println!("Ready. veil-warden tui / demo / status / stop");
    Ok(())
}
fn stop() -> Result {
    let _lock = operation_lock()?;
    for (role, port) in [("sandbox", 32223), ("builder", 32222)] {
        if loaded(role)? {
            if !Command::new("/bin/launchctl")
                .args(["bootout", &format!("{}/{}", domain()?, label(role))])
                .status()?
                .success()
            {
                return Err(format!("Cannot stop {role}").into());
            }
            let deadline = Instant::now() + Duration::from_secs(15);
            while reachable(port) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(100));
            }
            if reachable(port) {
                return Err(format!(
                    "{role} service removed, but port {port} is still occupied; inspect status"
                )
                .into());
            }
            println!("Stopped managed {role} VM");
        } else if reachable(port) {
            eprintln!(
                "{role} is running outside this CLI; preserved. Stop it in its original terminal (Ctrl+C)."
            );
        }
    }
    Ok(())
}
fn main() -> Result {
    if env::consts::OS != "macos" || env::consts::ARCH != "aarch64" {
        return Err("Host CLI requires an Apple Silicon Mac".into());
    }
    let args: Vec<String> = env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("tui");
    let rest = args.get(1..).unwrap_or(&[]);
    if cmd == "__vm" {
        use std::os::unix::process::CommandExt;
        let name = match rest {
            [role] if role == "builder" => "bootstrap-builder.sh",
            [role] if role == "sandbox" => "run-vm.sh",
            _ => return Err("Invalid VM role".into()),
        };
        return Err(script(name, &[]).exec().into());
    }
    match cmd {
        "--help" | "-h" | "help" if rest.is_empty() => println!("{HELP}"),
        "start" if rest.is_empty() => start()?,
        "tui" if rest.is_empty() || rest == ["--enforce"] => {
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                return Err("Run veil-warden tui in an interactive terminal".into());
            }
            start()?;
            run("tui-vm.sh", rest)?;
        }
        "demo" if rest.is_empty() => {
            start()?;
            run("demo-policy-vm.sh", &[])?;
        }
        "build" if rest.is_empty() => {
            let _lock = operation_lock()?;
            if reachable(32223) || loaded("sandbox")? {
                return Err(
                    "Stop sandbox before rebuilding (veil-warden stop for managed VMs)".into(),
                );
            }
            ensure_builder()?;
            run("build-vm.sh", &[])?;
            run("build-counter-vm.sh", &[])?;
        }
        "status" if rest.is_empty() => {
            for (role, port) in [("builder", 32222), ("sandbox", 32223)] {
                println!("{role}: SSH={} managed={}", reachable(port), loaded(role)?);
            }
            println!("Checkout: {REPO}\nState: {}", state().display());
        }
        "stop" if rest.is_empty() => stop()?,
        _ => return Err(format!("Invalid command/arguments\n{HELP}").into()),
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plist_escapes_paths_and_does_not_autorestart() {
        let p = plist("sandbox", "/a&b/<cli>", "/cache's", "/bin:\"path\"");
        assert!(p.contains("/a&amp;b/&lt;cli&gt;"));
        assert!(p.contains("/cache&apos;s"));
        assert!(p.contains("/bin:&quot;path&quot;"));
        assert!(p.contains("<key>KeepAlive</key><false/>"));
        assert!(p.contains("<string>__vm</string>"));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::*;
    #[test]
    #[ignore = "requires a local macOS GUI launchd session"]
    fn launchd_background_job_starts_and_stops() -> Result {
        let role = format!("smoke-{}", std::process::id());
        let dir = PathBuf::from(REPO).join("target/host-cli");
        fs::create_dir_all(&dir)?;
        let p = dir.join(format!("{role}.plist"));
        let cache = fs::canonicalize(&dir)?;
        fs::create_dir_all(cache.join("host-cli"))?;
        let document = plist(
            &role,
            "/bin/sleep",
            cache.to_str().unwrap(),
            "/usr/bin:/bin",
        )
        .replace(
            &format!("<string>__vm</string><string>{role}</string>"),
            "<string>60</string>",
        );
        fs::write(&p, document)?;
        assert!(
            Command::new("/usr/bin/plutil")
                .arg("-lint")
                .arg(&p)
                .status()?
                .success()
        );
        assert!(
            Command::new("/bin/launchctl")
                .args(["bootstrap", &domain()?])
                .arg(&p)
                .status()?
                .success()
        );
        let observed = loaded(&role);
        let stopped = Command::new("/bin/launchctl")
            .args(["bootout", &format!("{}/{}", domain()?, label(&role))])
            .status()?;
        assert!(observed?);
        assert!(stopped.success());
        assert!(!loaded(&role)?);
        Ok(())
    }
}
