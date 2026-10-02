// SPDX-License-Identifier: MIT
use std::{collections::BTreeSet, fs, path::Path};
use syn::visit::{self, Visit};
use toml::Value;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}

fn read_toml(path: &Path) -> Value {
    toml::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn validate_sandbox(value: &Value) -> Result<(), &'static str> {
    let m0 = &value["m0"];
    if m0["mode"].as_str() != Some("observe") || m0["attach_enabled"].as_bool() != Some(false) {
        return Err("M0 cannot attach or enforce");
    }
    if m0["scope"].as_str() != Some("/warden.slice/warden-test.slice") {
        return Err("scope must be the dedicated test slice");
    }
    if m0["ssh_bind"].as_str() != Some("127.0.0.1") || m0["ssh_port"].as_integer() != Some(32223) {
        return Err("management must use the dedicated loopback port");
    }
    for (key, range) in [("cpus", 1..=2), ("memory_mib", 1024..=4096)] {
        if !m0[key].as_integer().is_some_and(|v| range.contains(&v)) {
            return Err("resource limit exceeded");
        }
    }
    Ok(())
}

#[test]
fn m0_stays_observe_only_and_scoped() {
    validate_sandbox(&read_toml(&root().join("config/sandbox.toml"))).unwrap();
}

#[test]
fn policy_guard_rejects_dangerous_changes() {
    let initial = read_toml(&root().join("config/sandbox.toml"));
    for (key, invalid) in [
        ("attach_enabled", Value::Boolean(true)),
        ("mode", Value::String("enforce".into())),
        ("scope", Value::String("/".into())),
        ("scope", Value::String("/system.slice".into())),
        ("ssh_bind", Value::String("0.0.0.0".into())),
        ("cpus", Value::Integer(8)),
        ("memory_mib", Value::Integer(32768)),
    ] {
        let mut altered = initial.clone();
        altered["m0"][key] = invalid;
        assert!(
            validate_sandbox(&altered).is_err(),
            "allowed unsafe change to {key}"
        );
    }
}

fn dependency_packages(value: &Value, output: &mut BTreeSet<String>) {
    if let Some(table) = value.as_table() {
        for (key, item) in table {
            if matches!(
                key.as_str(),
                "dependencies" | "dev-dependencies" | "build-dependencies"
            ) {
                for (name, spec) in item.as_table().unwrap() {
                    output.insert(
                        spec.get("package")
                            .and_then(Value::as_str)
                            .unwrap_or(name)
                            .to_string(),
                    );
                }
            } else if item.is_table() {
                dependency_packages(item, output);
            }
        }
    }
}

fn validate_edges(
    name: &str,
    dependencies: &BTreeSet<String>,
    layers: &Value,
) -> Result<(), String> {
    let permitted = layers[name].as_array().unwrap();
    for dependency in dependencies {
        if layers.get(dependency).is_some()
            && !permitted
                .iter()
                .any(|allowed| allowed.as_str() == Some(dependency))
        {
            return Err(format!("forbidden workspace edge: {name} -> {dependency}"));
        }
    }
    Ok(())
}

#[test]
fn workspace_dependencies_follow_layer_direction() {
    let workspace = read_toml(&root().join("Cargo.toml"));
    let architecture = read_toml(&root().join("config/architecture.toml"));
    let layers = &architecture["layers"];
    for member in workspace["workspace"]["members"].as_array().unwrap() {
        let manifest = read_toml(&root().join(member.as_str().unwrap()).join("Cargo.toml"));
        let name = manifest["package"]["name"].as_str().unwrap();
        assert!(
            layers.get(name).is_some(),
            "unreviewed workspace member: {name}"
        );
        let mut dependencies = BTreeSet::new();
        dependency_packages(&manifest, &mut dependencies);
        if let Some(shared) = workspace["workspace"].get("dependencies") {
            for (key, value) in shared.as_table().unwrap() {
                if dependencies.remove(key) {
                    dependencies.insert(
                        value
                            .get("package")
                            .and_then(Value::as_str)
                            .unwrap_or(key)
                            .to_string(),
                    );
                }
            }
        }
        validate_edges(name, &dependencies, layers).unwrap();
    }
}

#[test]
fn renamed_and_target_specific_forbidden_edges_are_detected() {
    let layers = read_toml(&root().join("config/architecture.toml"));
    let manifest: Value = toml::from_str(
        r#"
        [target.'cfg(target_os = "linux")'.build-dependencies]
        hidden = { package = "veil-warden", path = "../veil-warden" }
    "#,
    )
    .unwrap();
    let mut dependencies = BTreeSet::new();
    dependency_packages(&manifest, &mut dependencies);
    assert!(validate_edges("veil-warden-ebpf", &dependencies, &layers["layers"]).is_err());
}

struct StdReferences(Vec<String>);
impl<'ast> Visit<'ast> for StdReferences {
    fn visit_use_name(&mut self, node: &'ast syn::UseName) {
        if matches!(node.ident.to_string().as_str(), "std" | "alloc") {
            self.0.push(node.ident.to_string());
        }
        visit::visit_use_name(self, node);
    }
    fn visit_use_rename(&mut self, node: &'ast syn::UseRename) {
        if matches!(node.ident.to_string().as_str(), "std" | "alloc") {
            self.0.push(node.ident.to_string());
        }
        visit::visit_use_rename(self, node);
    }
    fn visit_use_path(&mut self, node: &'ast syn::UsePath) {
        if matches!(node.ident.to_string().as_str(), "std" | "alloc") {
            self.0.push(node.ident.to_string());
        }
        visit::visit_use_path(self, node);
    }
    fn visit_path(&mut self, node: &'ast syn::Path) {
        if let Some(segment) = node.segments.first()
            && matches!(segment.ident.to_string().as_str(), "std" | "alloc")
        {
            self.0.push(segment.ident.to_string());
        }
        visit::visit_path(self, node);
    }
    fn visit_item_extern_crate(&mut self, node: &'ast syn::ItemExternCrate) {
        if matches!(node.ident.to_string().as_str(), "std" | "alloc") {
            self.0.push(node.ident.to_string());
        }
        visit::visit_item_extern_crate(self, node);
    }
}

fn rust_sources(path: &Path, output: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        assert!(
            !kind.is_symlink(),
            "source symlinks require explicit review"
        );
        if kind.is_dir() {
            rust_sources(&entry.path(), output);
        } else if entry.path().extension().is_some_and(|v| v == "rs") {
            output.push(entry.path());
        }
    }
}

#[test]
fn future_kernel_and_common_sources_stay_no_std() {
    for name in ["veil-warden-common", "veil-warden-ebpf"] {
        let source = root().join("crates").join(name).join("src");
        if !source.exists() {
            continue;
        } // No runtime crate is claimed at M0.
        let entry = source.join(if name.ends_with("common") {
            "lib.rs"
        } else {
            "main.rs"
        });
        let syntax = syn::parse_file(&fs::read_to_string(entry).unwrap()).unwrap();
        assert!(
            syntax.attrs.iter().any(|a| a.path().is_ident("no_std")),
            "{name} needs no_std"
        );
        let mut paths = Vec::new();
        rust_sources(&source, &mut paths);
        for path in paths {
            let syntax = syn::parse_file(&fs::read_to_string(&path).unwrap()).unwrap();
            let mut references = StdReferences(Vec::new());
            references.visit_file(&syntax);
            assert!(
                references.0.is_empty(),
                "std/alloc referenced in {}",
                path.display()
            );
        }
    }
}

#[test]
fn source_guard_parses_code_instead_of_comments() {
    let mut references = StdReferences(Vec::new());
    references.visit_file(
        &syn::parse_file("// std::net is not code\nextern crate alloc; use std::net::TcpStream;")
            .unwrap(),
    );
    assert_eq!(references.0.len(), 2);
}

#[test]
fn runtime_crates_cannot_escape_workspace_guards() {
    let workspace = read_toml(&root().join("Cargo.toml"));
    let members = workspace["workspace"]["members"].as_array().unwrap();
    for name in ["veil-warden-common", "veil-warden-ebpf", "veil-warden"] {
        let member = format!("crates/{name}");
        if root().join(&member).join("Cargo.toml").exists() {
            assert!(
                members.iter().any(|v| v.as_str() == Some(&member)),
                "unregistered runtime crate: {name}"
            );
        }
    }
}

fn linux_loader_declared(file: &syn::File) -> bool {
    file.items.iter().any(|item| {
        let syn::Item::Mod(module) = item else {
            return false;
        };
        module.ident == "loader"
            && module.attrs.iter().any(|attribute| {
                if !attribute.path().is_ident("cfg") {
                    return false;
                }
                let Ok(syn::Meta::NameValue(value)) = attribute.parse_args::<syn::Meta>() else {
                    return false;
                };
                let syn::Expr::Lit(expression) = value.value else {
                    return false;
                };
                let syn::Lit::Str(literal) = expression.lit else {
                    return false;
                };
                value.path.is_ident("target_os") && literal.value() == "linux"
            })
    })
}

#[test]
fn future_loader_is_gated_to_linux() {
    let src = root().join("crates/veil-warden/src");
    if !src.join("loader.rs").exists() {
        return;
    }
    let mut gated = false;
    for name in ["main.rs", "lib.rs"] {
        let path = src.join(name);
        if path.exists() {
            gated |= linux_loader_declared(
                &syn::parse_file(&fs::read_to_string(path).unwrap()).unwrap(),
            );
        }
    }
    assert!(
        gated,
        "loader needs an explicit cfg(target_os = linux) module boundary"
    );
}

#[test]
fn loader_guard_rejects_cross_platform_declarations() {
    for invalid in [
        "mod loader;",
        "#[cfg(target_os = \"macos\")] mod loader;",
        "#[cfg(any(target_os = \"linux\", target_os = \"macos\"))] mod loader;",
    ] {
        assert!(!linux_loader_declared(&syn::parse_file(invalid).unwrap()));
    }
    assert!(linux_loader_declared(
        &syn::parse_file("#[cfg(target_os = \"linux\")] mod loader;").unwrap()
    ));
}
