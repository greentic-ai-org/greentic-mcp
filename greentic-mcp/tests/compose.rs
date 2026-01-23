use greentic_mcp::compose::compose_router_with_bundled_adapter;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn compose_invokes_wasm_tools() {
    let temp = tempfile::tempdir().expect("tempdir");
    let args_log = temp.path().join("args.txt");
    // Safety: test only mutates process env within its own scope.
    unsafe {
        std::env::set_var("GREENTIC_MCP_TEST_ARGS", &args_log);
    }

    let wasm_tools = write_stub_wasm_tools(temp.path());
    let router = temp.path().join("router.wasm");
    let output = temp.path().join("out.component.wasm");
    fs::write(&router, b"router").expect("router write");

    compose_router_with_bundled_adapter(&router, &output, Some(&wasm_tools)).expect("compose ok");

    let args = fs::read_to_string(&args_log).expect("args log");
    assert!(args.contains("compose"), "missing compose subcommand");
    assert!(
        args.contains("router.wasm"),
        "missing router path in args: {args}"
    );
    assert!(
        args.contains("out.component.wasm"),
        "missing output path in args: {args}"
    );
    assert!(output.exists(), "output file not created");

    // Safety: cleanup mirrors the test's own env mutation.
    unsafe {
        std::env::remove_var("GREENTIC_MCP_TEST_ARGS");
    }
}

#[cfg(unix)]
fn write_stub_wasm_tools(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("wasm-tools");
    let script = r#"#!/usr/bin/env bash
set -euo pipefail
echo "$@" > "$GREENTIC_MCP_TEST_ARGS"
out=""
while [[ $# -gt 0 ]]; do
  if [[ "$1" == "-o" || "$1" == "--output" ]]; then
    out="$2"
    shift 2
    continue
  fi
  shift
done
if [[ -n "$out" ]]; then
  echo "ok" > "$out"
fi
"#;
    fs::write(&path, script).expect("write stub");
    let mut perms = fs::metadata(&path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).expect("chmod stub");
    path
}

#[cfg(windows)]
fn write_stub_wasm_tools(dir: &Path) -> PathBuf {
    let path = dir.join("wasm-tools.cmd");
    let script = r#"@echo off
setlocal enabledelayedexpansion
set args=%*
echo %args% > %GREENTIC_MCP_TEST_ARGS%
set out=
:loop
if "%1"=="" goto end
if "%1"=="-o" (
  set out=%2
  shift
  shift
  goto loop
)
if "%1"=="--output" (
  set out=%2
  shift
  shift
  goto loop
)
shift
goto loop
:end
if not "%out%"=="" (
  echo ok > "%out%"
)
"#;
    fs::write(&path, script).expect("write stub");
    path
}
