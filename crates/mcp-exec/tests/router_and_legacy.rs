use std::path::PathBuf;
use std::process::Command;

use greentic_mcp_exec::{ExecConfig, ExecRequest, RuntimePolicy, ToolStore, VerifyPolicy};
use serde_json::json;

fn build_fixture(path: &str, crate_name: &str) -> Option<PathBuf> {
    let target_installed = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|list| list.lines().any(|l| l.trim() == "wasm32-wasip2"))
        .unwrap_or(false);

    if !target_installed {
        eprintln!("Skipping test; wasm32-wasip2 target not installed");
        return None;
    }

    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    let target_dir = crate_dir.join("target");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .arg("--target-dir")
        .arg(&target_dir)
        .current_dir(&crate_dir)
        .status();

    match status {
        Ok(status) if status.success() => Some(
            target_dir
                .join("wasm32-wasip2/release")
                .join(format!("{crate_name}.wasm")),
        ),
        _ => {
            eprintln!("Skipping test; failed to build {}", crate_name);
            None
        }
    }
}

#[test]
fn executes_router_world() {
    let Some(wasm_path) = build_fixture("tests/router_echo", "router_echo") else {
        return;
    };

    let cfg = ExecConfig {
        store: ToolStore::LocalDir(wasm_path.parent().expect("parent").to_path_buf()),
        security: VerifyPolicy {
            allow_unverified: true,
            ..Default::default()
        },
        runtime: RuntimePolicy::default(),
        http_enabled: false,
        secrets_store: None,
    };

    let req = ExecRequest {
        component: "router_echo".into(),
        action: "echo".into(),
        args: json!({"msg": "hi"}),
        tenant: None,
    };

    let value = greentic_mcp_exec::exec(req, &cfg).expect("router exec");
    assert!(value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    let text = value
        .pointer("/result/content/0/text")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(text.contains("\"msg\""));
}

#[test]
fn falls_back_to_legacy_exec() {
    let Some(wasm_path) = build_fixture("tests/legacy_exec", "legacy_exec") else {
        return;
    };
    let cfg = ExecConfig {
        store: ToolStore::LocalDir(wasm_path.parent().expect("parent").to_path_buf()),
        security: VerifyPolicy {
            allow_unverified: true,
            ..Default::default()
        },
        runtime: RuntimePolicy::default(),
        http_enabled: false,
        secrets_store: None,
    };

    let req = ExecRequest {
        component: "legacy_exec".into(),
        action: "anything".into(),
        args: json!({"k": "v"}),
        tenant: None,
    };

    let value = greentic_mcp_exec::exec(req, &cfg).expect("legacy exec");
    assert_eq!(value.get("k").and_then(|v| v.as_str()), Some("v"));
}
