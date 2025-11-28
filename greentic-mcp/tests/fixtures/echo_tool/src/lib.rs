use serde_json::Value;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

static FLAKY_ATTEMPTS: AtomicU32 = AtomicU32::new(0);

wit_bindgen::generate!({
    path: "wit",
    world: "tool",
});

struct EchoTool;

impl Guest for EchoTool {
    fn tool_invoke(input: String) -> String {
        let value: Value = serde_json::from_str(&input).expect("valid json input");

        if matches!(value.get("fail").and_then(Value::as_str), Some("transient")) {
            panic!("transient failure requested");
        }

        if let Some(ms) = value.get("sleep_ms").and_then(Value::as_u64) {
            thread::sleep(Duration::from_millis(ms));
        }

        if value.get("flaky").and_then(Value::as_bool) == Some(true) {
            let attempt = FLAKY_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
            if attempt.is_multiple_of(2) {
                panic!("flaky tool induced transient failure");
            }
        }

        serde_json::to_string(&value).expect("valid json output")
    }
}

export!(EchoTool);
