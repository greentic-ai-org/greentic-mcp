//! Host-side ToolMap management and WASIX/WASI execution bridge for Greentic MCP tools.

pub mod auth;
pub mod compose;
pub mod config;
pub mod executor;
pub mod protocol;
pub mod retry;
pub mod tool_map;
pub mod types;

pub use config::load_tool_map_config;
pub use executor::WasixExecutor;
pub use tool_map::ToolMap;
pub use types::{McpError, ToolInput, ToolMapConfig, ToolOutput, ToolRef};

use greentic_mcp_exec::{ExecConfig, ExecError, ExecRequest, RunnerError};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::time::sleep;
/// Invoke a tool by name using a [`ToolMap`] and [`WasixExecutor`].
pub async fn invoke_with_map(
    map: &ToolMap,
    executor: &WasixExecutor,
    name: &str,
    input_json: Value,
) -> Result<Value, McpError> {
    let tool = map.get(name)?;
    let input = ToolInput {
        payload: input_json,
    };
    let output = executor.invoke(tool, &input).await?;
    Ok(output.payload)
}

/// Convenience helper for loading a tool map from disk and building a [`ToolMap`].
pub fn load_tool_map(path: &std::path::Path) -> Result<ToolMap, McpError> {
    let config = load_tool_map_config(path)?;
    ToolMap::from_config(&config)
}

pub mod test_tools;

use std::time::Duration;

type ExecFn = dyn Fn(ExecRequest, &ExecConfig) -> Result<Value, ExecError> + Send + Sync;

pub async fn exec_with_retries(req: ExecRequest, cfg: &ExecConfig) -> Result<Value, ExecError> {
    exec_with_retries_with(req, cfg, Arc::new(greentic_mcp_exec::exec)).await
}

pub async fn exec_with_retries_backend<F>(
    req: ExecRequest,
    cfg: &ExecConfig,
    exec_fn: F,
) -> Result<Value, ExecError>
where
    F: Fn(ExecRequest, &ExecConfig) -> Result<Value, ExecError> + Send + Sync + 'static,
{
    exec_with_retries_with(req, cfg, Arc::new(exec_fn)).await
}

async fn exec_with_retries_with(
    mut req: ExecRequest,
    cfg: &ExecConfig,
    executor: Arc<ExecFn>,
) -> Result<Value, ExecError> {
    let max_attempts = cfg.runtime.max_attempts.max(1);

    for attempt in 1..=max_attempts {
        if let Some(tenant) = req.tenant.as_mut() {
            tenant.attempt = attempt - 1;
        }

        let req_clone = req.clone();
        let cfg_clone = cfg.clone();
        let executor = executor.clone();
        let attempt_result =
            tokio::task::spawn_blocking(move || executor(req_clone, &cfg_clone)).await;

        let exec_result = match attempt_result {
            Ok(result) => result,
            Err(err) => {
                return Err(ExecError::runner(
                    req.component.clone(),
                    RunnerError::Internal(format!("blocking exec failed: {err:?}")),
                ));
            }
        };

        match exec_result {
            Ok(value) => return Ok(value),
            Err(err) => {
                let should_retry = attempt < max_attempts && is_transient_error(&err);
                if !should_retry {
                    return Err(err);
                }
                let backoff = cfg
                    .runtime
                    .base_backoff
                    .checked_mul(attempt)
                    .unwrap_or(cfg.runtime.base_backoff);
                sleep(backoff).await;
            }
        }
    }

    unreachable!("retry loop should never exit without returning")
}

fn is_transient_error(err: &ExecError) -> bool {
    match err {
        ExecError::Runner { source, .. } => matches!(source, RunnerError::Timeout { .. }),
        ExecError::Tool { code, .. } => code.starts_with("transient."),
        _ => false,
    }
}

/// Test-only helpers that run native “tools” without Wasm.
pub enum TestBackend {
    NativeEcho,
    NativeFlaky,
    NativeTimeout(Duration),
}

pub fn exec_test_backend(
    backend: TestBackend,
    input: Value,
    cfg: &ExecConfig,
) -> Result<Value, ExecError> {
    use crate::test_tools::*;

    match backend {
        TestBackend::NativeEcho => {
            echo(&input).map_err(|message| tool_error("echo", "tool-invoke", "echo", message))
        }
        TestBackend::NativeFlaky => flaky_echo(&input)
            .map_err(|message| tool_error("echo-flaky", "tool-invoke", "transient.echo", message)),
        TestBackend::NativeTimeout(sleep) => {
            if sleep > cfg.runtime.per_call_timeout {
                Err(ExecError::runner(
                    "echo-timeout",
                    RunnerError::Timeout {
                        elapsed: cfg.runtime.per_call_timeout,
                    },
                ))
            } else {
                timeout_echo(&input, sleep).map_err(|message| {
                    tool_error("echo-timeout", "tool-invoke", "timeout", message)
                })
            }
        }
    }
}

fn tool_error(component: &str, action: &str, code: &str, message: String) -> ExecError {
    ExecError::tool_error(component, action, code, json!({ "message": message }))
}
