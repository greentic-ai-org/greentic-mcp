use anyhow::{Context, Result};
use greentic_types::{SecretFormat, SecretKey, SecretRequirement, SecretScope};
use serde_json::Value;
use tracing::warn;

use crate::{ExecConfig, ExecError, ExecRequest, exec};

#[cfg(feature = "describe-v1")]
const DESCRIBE_INTERFACE: &str = "greentic:component/describe-v1@1.0.0";
#[cfg(feature = "describe-v1")]
const DESCRIBE_EXPORT: &str = "greentic:component/describe-v1@1.0.0#describe-json";

#[derive(Debug)]
pub enum Maybe<T> {
    Data(T),
    Unsupported,
}

#[derive(Debug)]
pub struct ToolDescribe {
    pub describe_v1: Option<Value>,
    pub capabilities: Maybe<Vec<String>>,
    pub secrets: Maybe<Value>,
    pub config_schema: Maybe<Value>,
    pub secret_requirements: Vec<SecretRequirement>,
}

pub fn describe_tool(name: &str, cfg: &ExecConfig) -> Result<ToolDescribe> {
    #[cfg(feature = "describe-v1")]
    {
        if let Some(document) = try_describe_v1(name, cfg)? {
            let (secret_requirements, used_legacy) =
                secret_requirements(Some(&document), &Maybe::Unsupported);
            if used_legacy {
                warn!(
                    tool = name,
                    "legacy secrets descriptors were mapped; emit `secret_requirements` in describe-json"
                );
            }
            return Ok(ToolDescribe {
                describe_v1: Some(document),
                capabilities: Maybe::Unsupported,
                secrets: Maybe::Unsupported,
                config_schema: Maybe::Unsupported,
                secret_requirements,
            });
        }
    }

    fn try_action(name: &str, action: &str, cfg: &ExecConfig) -> Result<Maybe<Value>> {
        let req = ExecRequest {
            component: name.to_string(),
            action: action.to_string(),
            args: Value::Object(Default::default()),
            tenant: None,
        };

        match exec(req, cfg) {
            Ok(v) => Ok(Maybe::Data(v)),
            Err(ExecError::NotFound { .. }) => Ok(Maybe::Unsupported),
            Err(ExecError::Tool { code, payload, .. }) if code == "iface-error.not-found" => {
                let _ = payload;
                Ok(Maybe::Unsupported)
            }
            Err(e) => Err(e.into()),
        }
    }

    let capabilities_value = try_action(name, "capabilities", cfg)?;
    let secrets = try_action(name, "list_secrets", cfg)?;
    let config_schema = try_action(name, "config_schema", cfg)?;

    let capabilities = match capabilities_value {
        Maybe::Data(value) => {
            if let Some(arr) = value.as_array() {
                let list = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>();
                Maybe::Data(list)
            } else {
                Maybe::Data(Vec::new())
            }
        }
        Maybe::Unsupported => Maybe::Unsupported,
    };

    let (secret_requirements, used_legacy) = secret_requirements(None, &secrets);
    if used_legacy {
        warn!(
            tool = name,
            "legacy secrets descriptors were mapped; emit `secret_requirements` in tool metadata"
        );
    }

    Ok(ToolDescribe {
        describe_v1: None,
        capabilities,
        secrets,
        config_schema,
        secret_requirements,
    })
}

#[cfg(feature = "describe-v1")]
fn try_describe_v1(name: &str, cfg: &ExecConfig) -> Result<Option<Value>> {
    use wasmtime::component::{Component, Linker};
    use wasmtime::{Config, Engine, Store};

    let resolved =
        crate::resolve::resolve(name, &cfg.store).map_err(|err| ExecError::resolve(name, err))?;
    let verified = crate::verify::verify(name, resolved, &cfg.security)
        .map_err(|err| ExecError::verification(name, err))?;

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.async_support(false);
    config.epoch_interruption(true);

    let engine = Engine::new(&config)?;
    let component = match Component::from_binary(&engine, verified.resolved.bytes.as_ref()) {
        Ok(component) => component,
        Err(_) => return Ok(None),
    };
    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());

    let instance = match linker.instantiate(&mut store, &component) {
        Ok(instance) => instance,
        Err(_) => return Ok(None),
    };
    if instance
        .get_export(&mut store, None, DESCRIBE_INTERFACE)
        .is_none()
    {
        return Ok(None);
    }

    let func = match instance.get_typed_func::<(), (String,)>(&mut store, DESCRIBE_EXPORT) {
        Ok(func) => func,
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("unknown export") {
                return Ok(None);
            }
            return Err(err);
        }
    };

    let (raw,) = func.call(&mut store, ())?;
    let value: Value =
        serde_json::from_str(&raw).with_context(|| "describe-json returned invalid JSON")?;
    Ok(Some(value))
}

pub const RUNTIME_SENTINEL: &str = "runtime";

fn secret_requirements(
    describe_v1: Option<&Value>,
    secrets: &Maybe<Value>,
) -> (Vec<SecretRequirement>, bool) {
    if let Some(requirements) = describe_v1.and_then(|doc| doc.get("secret_requirements")) {
        let parsed = normalize_requirements(requirements);
        return (dedup(parsed), false);
    }

    if let Maybe::Data(value) = secrets {
        let parsed = normalize_requirements(value);
        return (dedup(parsed), true);
    }

    (Vec::new(), matches!(secrets, Maybe::Data(_)))
}

fn normalize_requirements(value: &Value) -> Vec<SecretRequirement> {
    if let Some(arr) = value.as_array() {
        return arr.iter().filter_map(parse_requirement).collect();
    }

    if let Some(obj) = value.as_object() {
        if let Some(reqs) = obj.get("secret_requirements").and_then(Value::as_array) {
            return reqs.iter().filter_map(parse_requirement).collect();
        }
        if let Some(reqs) = obj.get("secrets").and_then(Value::as_array) {
            return reqs.iter().filter_map(parse_requirement).collect();
        }
    }

    Vec::new()
}

fn parse_requirement(value: &Value) -> Option<SecretRequirement> {
    match value {
        Value::String(key) => {
            let key = SecretKey::new(key).ok()?;
            let mut req = SecretRequirement::default();
            req.key = key;
            req.required = true;
            req.scope = Some(runtime_scope());
            req.format = Some(default_format());
            Some(req)
        }
        Value::Object(obj) => {
            let key_raw = obj
                .get("key")
                .or_else(|| obj.get("name"))
                .and_then(Value::as_str)?;
            let key = SecretKey::new(key_raw).ok()?;
            let required = obj
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| {
                    !obj.get("optional")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                });

            let scope = parse_scope(obj.get("scope"))
                .or_else(|| parse_scope(Some(&Value::Object(obj.clone()))))
                .unwrap_or_else(runtime_scope);

            let format = obj
                .get("format")
                .and_then(Value::as_str)
                .and_then(parse_format)
                .unwrap_or_else(default_format);

            let description = obj
                .get("description")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);

            let schema = obj.get("schema").cloned();
            let examples = obj
                .get("examples")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(example_to_string)
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            let mut req = SecretRequirement::default();
            req.key = key;
            req.required = required;
            req.description = description;
            req.scope = Some(scope);
            req.format = Some(format);
            req.schema = schema;
            req.examples = examples;
            Some(req)
        }
        _ => None,
    }
}

fn parse_scope(value: Option<&Value>) -> Option<SecretScope> {
    let obj = value?.as_object()?;
    let env = obj.get("env").and_then(Value::as_str)?;
    let tenant = obj.get("tenant").and_then(Value::as_str)?;
    let team = obj
        .get("team")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    Some(SecretScope {
        env: env.to_owned(),
        tenant: tenant.to_owned(),
        team,
    })
}

fn parse_format(value: &str) -> Option<SecretFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "json" => Some(SecretFormat::Json),
        "text" => Some(SecretFormat::Text),
        "opaque" => Some(SecretFormat::Bytes),
        "binary" | "bytes" | "byte" | "bin" => Some(SecretFormat::Bytes),
        _ => None,
    }
}

fn runtime_scope() -> SecretScope {
    SecretScope {
        env: RUNTIME_SENTINEL.to_owned(),
        tenant: RUNTIME_SENTINEL.to_owned(),
        team: None,
    }
}

fn default_format() -> SecretFormat {
    SecretFormat::Text
}

fn example_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        other => serde_json::to_string(other).ok(),
    }
}

fn dedup(requirements: Vec<SecretRequirement>) -> Vec<SecretRequirement> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(requirements.len());
    for req in requirements {
        let scope_key = req
            .scope
            .as_ref()
            .map(|scope| (scope.env.clone(), scope.tenant.clone(), scope.team.clone()));
        if seen.insert((req.key.clone(), scope_key)) {
            out.push(req);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_describe_v1_secret_requirements() {
        let describe_v1 = json!({
            "name": "demo",
            "secret_requirements": [
                {
                    "key": "api-key",
                    "required": false,
                    "format": "json",
                    "scope": { "env": "dev", "tenant": "acme" },
                    "description": "auth key"
                }
            ]
        });

        let (reqs, used_legacy) = secret_requirements(Some(&describe_v1), &Maybe::Unsupported);
        assert!(!used_legacy);
        assert_eq!(reqs.len(), 1);
        let req = &reqs[0];
        assert_eq!(req.key.as_str(), "api-key");
        assert!(!req.required);
        assert_eq!(req.format, Some(SecretFormat::Json));
        let scope = req.scope.as_ref().expect("scope set");
        assert_eq!(scope.env, "dev");
        assert_eq!(scope.tenant, "acme");
        assert_eq!(req.description.as_deref(), Some("auth key"));
    }

    #[test]
    fn maps_legacy_list_secrets_strings() {
        let secrets = Maybe::Data(json!(["token", "secondary"]));
        let (reqs, used_legacy) = secret_requirements(None, &secrets);
        assert!(used_legacy);
        assert_eq!(reqs.len(), 2);

        for req in reqs {
            assert!(req.required);
            assert_eq!(req.format, Some(SecretFormat::Text));
            let scope = req.scope.as_ref().expect("scope set");
            assert_eq!(scope.env, RUNTIME_SENTINEL);
            assert_eq!(scope.tenant, RUNTIME_SENTINEL);
        }
    }
}
