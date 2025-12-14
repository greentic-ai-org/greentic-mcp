use greentic_mcp::{TestBackend, exec_test_backend, exec_with_retries_backend};
use mcp_exec::{ExecConfig, ExecRequest, RuntimePolicy, ToolStore, VerifyPolicy};
use serde_json::json;
use std::time::Duration;
use tempfile::tempdir;

fn default_runtime_policy() -> RuntimePolicy {
    RuntimePolicy {
        per_call_timeout: Duration::from_secs(10),
        max_attempts: 1,
        base_backoff: Duration::from_millis(50),
        ..RuntimePolicy::default()
    }
}

fn test_exec_config(runtime: RuntimePolicy) -> (ExecConfig, tempfile::TempDir) {
    let dir = tempdir().expect("tempdir");
    let cfg = ExecConfig {
        store: ToolStore::LocalDir(dir.path().into()),
        security: VerifyPolicy::default(),
        runtime,
        http_enabled: false,
        secrets_store: None,
    };
    (cfg, dir)
}

#[tokio::test]
async fn echo_ok() {
    let (cfg, _tmp) = test_exec_config(default_runtime_policy());
    let result = exec_test_backend(TestBackend::NativeEcho, json!({"hello": "world"}), &cfg)
        .expect("tool success");

    assert_eq!(result, json!({"hello": "world"}));
}

#[tokio::test]
async fn echo_timeout() {
    let mut runtime = default_runtime_policy();
    runtime.per_call_timeout = Duration::from_millis(200);
    let (cfg, _tmp) = test_exec_config(runtime);

    let err = exec_test_backend(
        TestBackend::NativeTimeout(Duration::from_millis(400)),
        json!({"sleep_ms": 500, "note": "slow"}),
        &cfg,
    )
    .expect_err("should timeout");

    match err {
        mcp_exec::ExecError::Runner { source, .. } => match source {
            mcp_exec::RunnerError::Timeout { .. } => {}
            other => panic!("expected timeout error, got {other:?}"),
        },
        other => panic!("expected runner timeout, got {other:?}"),
    }
}

#[tokio::test]
async fn echo_transient_retries() {
    let mut runtime = default_runtime_policy();
    runtime.per_call_timeout = Duration::from_secs(3);
    runtime.max_attempts = 5;
    runtime.base_backoff = Duration::from_millis(50);
    let (cfg, _tmp) = test_exec_config(runtime);

    let req = ExecRequest {
        component: "echo-flaky".into(),
        action: "tool-invoke".into(),
        args: json!({"flaky": true, "message": "hello"}),
        tenant: None,
    };

    let result = exec_with_retries_backend(req, &cfg, |req, cfg| {
        exec_test_backend(TestBackend::NativeFlaky, req.args, cfg)
    })
    .await
    .expect("flaky tool should eventually succeed");

    assert_eq!(result, json!({"flaky": true, "message": "hello"}));
}
