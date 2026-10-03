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
#[derive(Clone, Copy)]
enum Language {
    Japanese,
    English,
}

impl Language {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "ja" => Ok(Self::Japanese),
            "en" => Ok(Self::English),
            _ => Err(format!("Unsupported language '{value}'. Choose ja or en.").into()),
        }
    }
}

fn preferred_language() -> Language {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|key| env::var(key).ok())
        .find(|value| !value.is_empty())
        .is_some_and(|locale| locale.to_ascii_lowercase().starts_with("ja"))
        .then_some(Language::Japanese)
        .unwrap_or(Language::English)
}

fn print_help(language: Language, command: Option<&str>) {
    let text = match (language, command) {
        (Language::Japanese, None) => {
            "\
veil-warden — ローカルLinux VMで動く通信観測ツール

使い方:
  veil-warden [--lang ja|en] [コマンド] [オプション]
  veil-warden --help | -h [--lang ja|en]

コマンド:
  start             VMを起動し、必要なビルドと事前確認を実行
  tui               通信監視画面を開く（既定。監視のみ）
  tui --enforce     明示的な宛先ルールで新規TCP接続を拒否
  demo              合成通信で拒否・解除・復帰を実演
  build             VMと実行ファイルを再ビルド
  status            VMの起動状態を表示
  stop              このコマンドが起動したVMを停止

ヘルプ:
  veil-warden start -h       コマンドごとの詳しい説明
  veil-warden tui --help     コマンドごとの詳しい説明
  veil-warden --lang en -h   英語で表示

言語は --lang ja または --lang en で選べます。省略時は LC_ALL、
LC_MESSAGES、LANG を参照し、日本語ロケール以外では英語を表示します。
ヘルプ表示ではVMを起動しません。初回は依存の取得に時間がかかることがあります。"
        }
        (Language::English, None) => {
            "\
veil-warden — network monitoring in a local Linux VM

Usage:
  veil-warden [--lang ja|en] [COMMAND] [OPTIONS]
  veil-warden --help | -h [--lang ja|en]

Commands:
  start             Start VMs, build missing artifacts, and run preflight
  tui               Open the monitor (default; observe only)
  tui --enforce     Deny new TCP connects using explicit destination rules
  demo              Demonstrate deny, removal, and recovery with synthetic traffic
  build             Rebuild the VM and runtime artifacts
  status            Show VM reachability and management state
  stop              Stop VMs started by this command

Help:
  veil-warden start -h       Show detailed help for a command
  veil-warden tui --help     Show detailed help for a command
  veil-warden --lang ja -h   Display help in Japanese

Choose a language with --lang ja or --lang en. If omitted, the CLI reads
LC_ALL, LC_MESSAGES, then LANG. Japanese locales select Japanese; others select
English. Help never starts a VM. The first run may take time to download dependencies."
        }
        (Language::Japanese, Some("start")) => {
            "\
start — VMを起動して利用可能か確認します

使い方: veil-warden start

ビルド結果があれば再利用します。不足している場合は構築用VMを起動し、
専用VMと監視用プログラムをビルドします。その後、専用VMを起動し、
SSH接続と warden-preflight を確認します。初回はダウンロードとビルドに
時間がかかることがあります。VMはターミナルを閉じても動作します。
ログ: 状態ディレクトリ/host-cli/{builder,sandbox,preflight}.log
VMを止めるには veil-warden stop を実行します。"
        }
        (Language::English, Some("start")) => {
            "\
start — start the VMs and check that the sandbox is ready

Usage: veil-warden start

Existing build artifacts are reused. If any are missing, the command starts the
build VM and builds the sandbox and monitor. It then starts the sandbox and
checks SSH access and warden-preflight. The first run may take time to download
and build dependencies. VMs keep running after the terminal closes.
Logs: STATE_DIR/host-cli/{builder,sandbox,preflight}.log
Run veil-warden stop to stop VMs managed by this command."
        }
        (Language::Japanese, Some("tui")) => {
            "\
tui — 通信監視画面を開きます

使い方: veil-warden [--lang ja|en] tui [--enforce]

--enforce を省略すると監視のみです。指定すると、確認した宛先への新規TCP
接続だけを専用テスト用cgroup内で拒否できます。UDP、既存接続、VM外の
プロセスには適用しません。TUIでは b で拒否、d で解除を選び、表示された
宛先を確認してEnterで実行します。Escは取消、q/Ctrl+Cは画面を閉じます。
使い方: veil-warden tui --enforce
終了すると監視プログラムは停止します。VMは起動したままです。"
        }
        (Language::English, Some("tui")) => {
            "\
tui — open the connection monitor

Usage: veil-warden [--lang ja|en] tui [--enforce]

Without --enforce, the monitor observes only. With --enforce, it can deny new
TCP connects to confirmed destinations inside the dedicated test cgroup. It
does not affect UDP, existing connections, or processes outside the VM. In the
TUI, press b to prepare a deny rule or d to prepare removal, review the displayed
destination, then press Enter to apply. Esc cancels; q/Ctrl+C exits.
Example: veil-warden tui --enforce
Exiting stops the monitor. The VM stays running."
        }
        (Language::Japanese, Some("demo")) => {
            "\
demo — 合成loopback通信で動作を実演します

使い方: veil-warden demo

専用VMを起動・確認した後、許可、拒否、ルール解除、再拒否、終了後の復帰を
合成TCP通信で確認します。実ネットワークや任意のアプリは対象にしません。
ヘルプ: veil-warden demo -h"
        }
        (Language::English, Some("demo")) => {
            "\
demo — demonstrate policy behavior with synthetic loopback traffic

Usage: veil-warden demo

Starts and checks the sandbox, then demonstrates allowed traffic, denial, rule
removal, denial again, and recovery after exit using synthetic TCP traffic. It
does not target arbitrary applications or external networks.
Help: veil-warden demo -h"
        }
        (Language::Japanese, Some("build")) => {
            "\
build — VMと監視用プログラムを再ビルドします

使い方: veil-warden build

専用VMを停止してから実行してください。構築用VMを利用し、現在のcheckout
からVMと実行ファイルを作り直します。既存の成果物やログは削除しません。
初回やソース変更後は時間がかかることがあります。
例: veil-warden stop && veil-warden build"
        }
        (Language::English, Some("build")) => {
            "\
build — rebuild the sandbox and monitor artifacts

Usage: veil-warden build

Stop the sandbox before rebuilding. The command uses the build VM and builds
from the current checkout. Existing artifacts and logs are kept. Initial builds
and builds after source changes may take time.
Example: veil-warden stop && veil-warden build"
        }
        (Language::Japanese, Some("status")) => {
            "\
status — VMの接続状態とCLIによる管理状態を表示します

使い方: veil-warden status

構築用VMと専用VMのloopback SSHポートの応答、およびこのCLIが管理する
VMジョブの状態を表示します。ポート応答だけでは、VM内部の機能が正常とは
判定できません。詳しい確認には veil-warden start を実行してください。
状態ディレクトリも表示します。"
        }
        (Language::English, Some("status")) => {
            "\
status — show VM connectivity and CLI management state

Usage: veil-warden status

Shows whether the builder and sandbox SSH ports on loopback respond, whether
this CLI manages their launchd jobs, and the state directory. A responding port
alone does not confirm the VM is healthy. Run veil-warden start for preflight."
        }
        (Language::Japanese, Some("stop")) => {
            "\
stop — このCLIが起動したVMを停止します

使い方: veil-warden stop

このCLIが管理している構築用VM・専用VMだけを停止します。手動で起動した
VMは停止せず、元のターミナルで停止するよう案内します。監視画面だけを
閉じる場合は q または Ctrl+C を押します。stop はビルド成果物・ログを
削除しません。"
        }
        (Language::English, Some("stop")) => {
            "\
stop — stop VMs started by this CLI

Usage: veil-warden stop

Stops only builder or sandbox VMs managed by this CLI. VMs started manually are
left running; stop those in their original terminal. To close only the monitor,
press q or Ctrl+C. This command does not delete build artifacts or logs."
        }
        _ => {
            print_help(language, None);
            return;
        }
    };
    println!("{text}");
}
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

fn parse_invocation(args: Vec<String>) -> Result<(Language, bool, Vec<String>)> {
    let mut language = None;
    let mut show_help = false;
    let mut command_args = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lang" | "--language" => {
                let value = args.next().ok_or("--lang requires ja or en")?;
                language = Some(Language::parse(&value)?);
            }
            "-h" | "--help" => show_help = true,
            _ => command_args.push(arg),
        }
    }
    Ok((
        language.unwrap_or_else(preferred_language),
        show_help,
        command_args,
    ))
}

fn main() -> Result {
    if env::consts::OS != "macos" || env::consts::ARCH != "aarch64" {
        return Err("Host CLI requires an Apple Silicon Mac".into());
    }
    let (language, show_help, args) = parse_invocation(env::args().skip(1).collect())?;
    let help_alias = args.first().is_some_and(|arg| arg == "help");
    if show_help || help_alias {
        let command = if help_alias {
            args.get(1).map(String::as_str)
        } else {
            args.first().map(String::as_str)
        };
        print_help(language, command);
        return Ok(());
    }
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
        _ => {
            return Err(match language {
                Language::Japanese => {
                    "コマンドまたはオプションが正しくありません。veil-warden -h を参照してください."
                        .into()
                }
                Language::English => "Unknown command or option. See veil-warden -h.".into(),
            });
        }
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
