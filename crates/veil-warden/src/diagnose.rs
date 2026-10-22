// SPDX-License-Identifier: MIT
//! Read-only preliminary checks. Missing evidence never implies compatibility.
use std::{fs::File, io::Read, path::Path};
const TEST_SCOPE: &str = "/warden.slice/warden-test.slice";
const LIMIT: u64 = 65536;

pub fn help() {
    println!(
        "veil-warden diagnose --cgroup /slice/app.service [--lang ja|en]\nRead-only: no BPF load or configuration changes.\n読み取り専用：BPFロード・設定変更なし。\nExit: 1=blocked / 不適合, 2=invalid input / 入力不正, 3=unverified / 未確認.\nNo deployment-ready result is emitted / 導入可能とは判定しません。"
    );
}
#[derive(Debug, PartialEq, Eq)]
struct Request {
    cgroup: String,
    japanese: bool,
}
fn parse(args: &[String]) -> Result<Request, &'static str> {
    let mut cgroup = None;
    let mut language = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cgroup" if cgroup.is_none() => {
                cgroup = Some(
                    args.next()
                        .ok_or("missing cgroup / cgroupが必要です")?
                        .clone(),
                )
            }
            "--lang" if language.is_none() => {
                language = Some(
                    args.next()
                        .ok_or("missing language / 言語が必要です")?
                        .clone(),
                )
            }
            _ => return Err("invalid or duplicate option / 不正または重複したオプション"),
        }
    }
    let cgroup = cgroup.ok_or("--cgroup is required / --cgroup が必要です")?;
    validate_scope(&cgroup)?;
    let japanese = match language.as_deref().unwrap_or("ja") {
        "ja" => true,
        "en" => false,
        _ => return Err("--lang must be ja or en / 言語は ja または en"),
    };
    Ok(Request { cgroup, japanese })
}
fn validate_scope(scope: &str) -> Result<(), &'static str> {
    if !scope.starts_with('/') || scope.len() > 1024 || scope.ends_with('/') {
        return Err("invalid cgroup path / cgroupパスが不正です");
    }
    let components: Vec<_> = scope[1..].split('/').collect();
    if components.iter().any(|c| {
        c.is_empty()
            || *c == "."
            || *c == ".."
            || !c
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.@:-".contains(&b))
    }) {
        return Err("unsupported cgroup path / 未対応のcgroupパスです");
    }
    let leaf = components.last().ok_or("empty scope")?;
    if !leaf.ends_with(".service") && scope != TEST_SCOPE {
        return Err(
            "select one service, not a parent slice / 親sliceではなく1サービスを指定してください",
        );
    }
    if components.iter().any(|c| {
        matches!(*c, "ssh.service" | "sshd.service" | "init.scope")
            || c.starts_with("veil-warden")
            || c.starts_with("ssh@")
            || c.starts_with("sshd@")
    }) {
        return Err("management/self scope is excluded / 管理経路・自身は対象外です");
    }
    Ok(())
}
fn read_bounded(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > LIMIT {
        return None;
    }
    String::from_utf8(bytes).ok()
}
fn kernel_supported(release: &str) -> Option<bool> {
    let mut parts = release.trim().split('.');
    Some(
        (
            parts.next()?.parse::<u32>().ok()?,
            parts.next()?.parse::<u32>().ok()?,
        ) >= (5, 8),
    )
}
#[derive(Debug)]
struct Check {
    id: &'static str,
    state: &'static str,
    en: &'static str,
    ja: &'static str,
}
fn evaluate(
    linux: bool,
    arch: &str,
    kernel: Option<&str>,
    v2: bool,
    scope_valid: bool,
    hostname: Option<&str>,
    scope: &str,
) -> Vec<Check> {
    vec![
        Check {
            id: "platform",
            state: if linux && matches!(arch, "x86_64" | "aarch64") {
                "candidate"
            } else {
                "blocked"
            },
            en: "Linux x86_64/aarch64 required; runtime support is not certified",
            ja: "Linux x86_64/aarch64が必要。実動作の対応認定ではありません",
        },
        Check {
            id: "kernel_version",
            state: match kernel.and_then(kernel_supported) {
                Some(true) => "candidate",
                Some(false) => "blocked",
                None => "unknown",
            },
            en: "Version floor only; BPF configuration and privileges remain unverified",
            ja: "バージョン下限のみ。BPF設定・権限の適合性は未確認です",
        },
        Check {
            id: "cgroup_v2",
            state: if v2 { "candidate" } else { "unknown" },
            en: "Read-only marker check; no mounts or controller changes",
            ja: "読み取りによる目印の確認のみ。mountやcontrollerは変更しません",
        },
        Check {
            id: "target_directory",
            state: if scope_valid { "candidate" } else { "unknown" },
            en: "Existing non-symlink directory; service ownership/lifecycle unverified",
            ja: "既存の非symlinkディレクトリ。サービスとの対応・寿命は未確認です",
        },
        Check {
            id: "loader_scope",
            state: if hostname.map(str::trim) == Some("veil-warden-sandbox") && scope == TEST_SCOPE
            {
                "candidate"
            } else {
                "blocked"
            },
            en: "Current loader permits only the dedicated sandbox hostname and fixed test cgroup",
            ja: "現行loaderは専用sandboxのhostnameと固定テストcgroupのみ許可します",
        },
        Check {
            id: "runtime_acceptance",
            state: "unknown",
            en: "No BPF loaded: features, permissions, LSM, competing attachments and stop recovery unverified",
            ja: "BPF未ロード：機能・権限・LSM・既存attach・停止復帰は未確認です",
        },
    ]
}
fn non_symlink_directory(mount: &Path, scope: &str) -> bool {
    let mut path = mount.to_path_buf();
    if !std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir() && !m.is_symlink()) {
        return false;
    }
    for part in scope[1..].split('/') {
        path.push(part);
        if !std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir() && !m.is_symlink()) {
            return false;
        }
    }
    true
}
pub fn run(args: &[String]) -> i32 {
    if args == ["--help"] || args == ["-h"] {
        help();
        return 0;
    }
    let request = match parse(args) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let linux = cfg!(target_os = "linux");
    let kernel = if linux {
        read_bounded(Path::new("/proc/sys/kernel/osrelease"))
    } else {
        None
    };
    let hostname = if linux {
        read_bounded(Path::new("/etc/hostname"))
    } else {
        None
    };
    let mount = Path::new("/sys/fs/cgroup");
    let checks = evaluate(
        linux,
        std::env::consts::ARCH,
        kernel.as_deref(),
        linux && read_bounded(&mount.join("cgroup.controllers")).is_some(),
        linux && non_symlink_directory(mount, &request.cgroup),
        hostname.as_deref(),
        &request.cgroup,
    );
    println!(
        "schema=veil-warden-preflight-v1\nread_only=true\nbpf_loaded=false\napply_supported=false\ndeployment_ready=false"
    );
    for check in &checks {
        println!(
            "{}={} {}",
            check.id,
            check.state,
            if request.japanese { check.ja } else { check.en }
        );
    }
    if checks.iter().any(|c| c.state == "blocked") {
        1
    } else {
        3
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_wide_traversal_management_and_injected_paths() {
        for path in [
            "/",
            "system.slice/app.service",
            "/system.slice",
            "/a/../x.service",
            "/a//x.service",
            "/a/ssh.service",
            "/a/ssh@1.service",
            "/a/veil-warden.service",
            "/a/x.service\n",
            "/a/x.service/",
        ] {
            assert!(validate_scope(path).is_err(), "{path:?}");
        }
        assert!(validate_scope("/system.slice/app.service").is_ok());
        assert!(validate_scope(TEST_SCOPE).is_ok());
    }
    #[test]
    fn input_is_explicit_and_strict() {
        let args = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        for s in [
            "",
            "--cgroup",
            "--cgroup /a/x.service --lang",
            "--cgroup /a/x.service --lang fr",
            "--cgroup /a/x.service --enforce",
            "--cgroup /a/x.service --cgroup /b/y.service",
        ] {
            assert!(parse(&args(s)).is_err(), "{s}");
        }
        assert!(parse(&args("--cgroup /a/x.service --lang en")).is_ok());
    }
    #[test]
    fn version_floor_is_not_runtime_acceptance() {
        assert_eq!(kernel_supported("5.7.9"), Some(false));
        assert_eq!(kernel_supported("5.8.0-custom"), Some(true));
        assert_eq!(kernel_supported("6.12.1"), Some(true));
        assert_eq!(kernel_supported("unknown"), None);
        let checks = evaluate(
            true,
            "aarch64",
            Some("6.12.0"),
            true,
            true,
            Some("veil-warden-sandbox\n"),
            TEST_SCOPE,
        );
        assert!(!checks.iter().any(|c| c.state == "blocked"));
        assert!(
            checks
                .iter()
                .any(|c| c.id == "runtime_acceptance" && c.state == "unknown")
        );
    }
    #[test]
    fn generic_server_stays_blocked_and_missing_evidence_stays_unknown() {
        let checks = evaluate(
            true,
            "x86_64",
            None,
            false,
            false,
            Some("production"),
            "/system.slice/app.service",
        );
        assert!(
            checks
                .iter()
                .any(|c| c.id == "loader_scope" && c.state == "blocked")
        );
        for id in ["kernel_version", "cgroup_v2", "target_directory"] {
            assert!(checks.iter().any(|c| c.id == id && c.state == "unknown"));
        }
        assert!(
            evaluate(false, "aarch64", None, false, false, None, TEST_SCOPE)
                .iter()
                .any(|c| c.id == "platform" && c.state == "blocked")
        );
    }

    #[test]
    fn filesystem_reads_are_bounded_and_symlink_targets_are_not_candidates() {
        // Retain fixtures; never remove pre-existing paths or evidence.
        let fixture = std::env::temp_dir().join(format!(
            "warden-preflight-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&fixture).unwrap();
        let text = fixture.join("text");
        std::fs::write(&text, vec![b'x'; LIMIT as usize]).unwrap();
        assert_eq!(read_bounded(&text).unwrap().len(), LIMIT as usize);
        std::fs::write(&text, vec![b'x'; LIMIT as usize + 1]).unwrap();
        assert!(read_bounded(&text).is_none());
        assert!(read_bounded(&fixture.join("missing")).is_none());
        std::fs::create_dir(fixture.join("app.service")).unwrap();
        assert!(non_symlink_directory(&fixture, "/app.service"));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(fixture.join("app.service"), fixture.join("link.service"))
                .unwrap();
            assert!(!non_symlink_directory(&fixture, "/link.service"));
            std::os::unix::fs::symlink(&fixture, fixture.join("mount-link")).unwrap();
            assert!(!non_symlink_directory(
                &fixture.join("mount-link"),
                "/app.service"
            ));
        }
    }
}
