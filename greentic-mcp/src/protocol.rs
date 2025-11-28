use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

const JSONRPC_2_0: &str = "2.0";

fn jsonrpc_version() -> String {
    JSONRPC_2_0.to_string()
}

/// Supported MCP protocol revisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub enum ProtocolRevision {
    #[serde(rename = "2025-03-26")]
    V2025_03_26,
    #[default]
    #[serde(rename = "2025-06-18")]
    V2025_06_18,
}

impl ProtocolRevision {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ProtocolRevision::V2025_03_26 => "2025-03-26",
            ProtocolRevision::V2025_06_18 => "2025-06-18",
        }
    }
}

impl Display for ProtocolRevision {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProtocolRevision {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "2025-03-26" | "v2025-03-26" | "2025_03_26" => Ok(ProtocolRevision::V2025_03_26),
            "2025-06-18" | "v2025-06-18" | "2025_06_18" | "2025-06" => {
                Ok(ProtocolRevision::V2025_06_18)
            }
            other => Err(format!(
                "unsupported protocol revision '{}'; expected 2025-03-26 or 2025-06-18",
                other
            )),
        }
    }
}

/// Per-server configuration for selecting protocol revisions.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_revision: Option<ProtocolRevision>,
    #[serde(default = "default_auth_mode")]
    pub auth_mode: AuthMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bearer_token: Option<String>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn default_auth_mode() -> AuthMode {
    AuthMode::None
}

impl McpServerConfig {
    pub fn resolved_protocol_revision(&self) -> ProtocolRevision {
        self.protocol_revision.unwrap_or_default()
    }

    /// Infer auth mode when `auth_mode` is left as default but other hints exist.
    pub fn resolved_auth_mode(&self) -> AuthMode {
        if self.auth_mode != AuthMode::None {
            return self.auth_mode;
        }
        if self.oauth.is_some() {
            return AuthMode::OAuth;
        }
        if self.api_key.is_some() {
            return AuthMode::ApiKey;
        }
        if self.bearer_token.is_some() {
            return AuthMode::BearerToken;
        }
        AuthMode::None
    }

    /// Validate protocol/auth pair, enforcing resource indicator for 2025-06.
    pub fn validate(&self) -> Result<(), String> {
        let rev = self.resolved_protocol_revision();
        if matches!(self.resolved_auth_mode(), AuthMode::OAuth) {
            let resource = self
                .oauth
                .as_ref()
                .and_then(|cfg| cfg.resource.as_deref())
                .unwrap_or("");
            if resource.is_empty() && rev == ProtocolRevision::V2025_06_18 {
                return Err(format!(
                    "server '{}' requires oauth.resource for protocol {}",
                    self.name,
                    rev.as_str()
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    None,
    ApiKey,
    BearerToken,
    #[serde(rename = "oauth")]
    OAuth,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct OAuthConfig {
    pub provider: String,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// JSON-RPC 2.0 request shape used by MCP.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct McpRequest<P = Value> {
    #[serde(default = "jsonrpc_version")]
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<P>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// JSON-RPC 2.0 response shape used by MCP.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct McpResponse<R = Value, E = RpcError> {
    #[serde(default = "jsonrpc_version")]
    pub jsonrpc: String,
    pub id: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<R>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<E>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// JSON-RPC notification shape used by MCP (no ID).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct McpNotification<P = Value> {
    #[serde(default = "jsonrpc_version")]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<P>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Protocol content wrapper. Flexible, passes through unknown fields.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Content {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// MCP tool schema.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "inputSchema"
    )]
    pub input_schema: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "outputSchema"
    )]
    pub output_schema: Option<Value>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolListResult {
    pub tools: Vec<Tool>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Tool call result payload.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CallToolResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<Content>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "isError")]
    pub is_error: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "structuredContent"
    )]
    pub structured_content: Option<Value>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Initialize request parameters; kept intentionally loose for compatibility.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InitializeParams {
    #[serde(rename = "protocol")]
    pub protocol_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capabilities: BTreeMap<String, Value>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Helper to build an initialize request with the correct revision string.
pub fn initialize_request_with_revision(
    id: Value,
    revision: ProtocolRevision,
    params_extra: BTreeMap<String, Value>,
) -> McpRequest<InitializeParams> {
    McpRequest {
        jsonrpc: jsonrpc_version(),
        id,
        method: "initialize".to_string(),
        params: Some(InitializeParams {
            protocol_version: revision.as_str().to_string(),
            client: None,
            capabilities: BTreeMap::new(),
            extra: params_extra,
        }),
        extra: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_protocol_revision_from_str() {
        assert_eq!(
            ProtocolRevision::from_str("2025-03-26").unwrap(),
            ProtocolRevision::V2025_03_26
        );
        assert_eq!(
            ProtocolRevision::from_str("2025-06-18").unwrap(),
            ProtocolRevision::V2025_06_18
        );
        assert!(ProtocolRevision::from_str("2024-01-01").is_err());
    }

    #[test]
    fn defaults_protocol_revision_when_missing_in_config() {
        let raw = r#"{ "name": "demo" }"#;
        let cfg: McpServerConfig = serde_json::from_str(raw).expect("parse config");
        assert_eq!(
            cfg.resolved_protocol_revision(),
            ProtocolRevision::V2025_06_18
        );
    }

    #[test]
    fn validates_oauth_resource_for_new_protocol() {
        let raw = r#"{
            "name": "svc",
            "protocol_revision": "2025-06-18",
            "auth_mode": "oauth",
            "oauth": { "provider": "auth0" }
        }"#;
        let cfg: McpServerConfig = serde_json::from_str(raw).expect("parse config");
        assert!(cfg.validate().is_err());

        let ok_raw = r#"{
            "name": "svc",
            "protocol_revision": "2025-06-18",
            "auth_mode": "oauth",
            "oauth": { "provider": "auth0", "resource": "https://svc" }
        }"#;
        let ok_cfg: McpServerConfig = serde_json::from_str(ok_raw).expect("parse config");
        assert!(ok_cfg.validate().is_ok());
    }

    #[test]
    fn initialize_requests_carry_revision() {
        let new_req = initialize_request_with_revision(
            json!(1),
            ProtocolRevision::V2025_06_18,
            BTreeMap::new(),
        );
        let old_req = initialize_request_with_revision(
            json!(1),
            ProtocolRevision::V2025_03_26,
            BTreeMap::new(),
        );

        let new_proto = new_req.params.as_ref().unwrap().protocol_version.clone();
        let old_proto = old_req.params.as_ref().unwrap().protocol_version.clone();
        assert_ne!(new_proto, old_proto);
        assert_eq!(new_proto, "2025-06-18");
        assert_eq!(old_proto, "2025-03-26");
    }

    #[test]
    fn tool_and_call_results_capture_optional_fields() {
        let list_json = json!({
            "tools": [
                {
                    "name": "echo",
                    "description": "echo message",
                    "inputSchema": {"type": "object"},
                    "outputSchema": {"type": "string"},
                    "x-extra": true
                }
            ],
            "meta": "ok"
        });

        let parsed: ToolListResult = serde_json::from_value(list_json).expect("parse tool list");
        assert_eq!(parsed.tools.len(), 1);
        let tool = &parsed.tools[0];
        assert_eq!(
            tool.output_schema
                .as_ref()
                .and_then(|v| v.get("type"))
                .and_then(Value::as_str),
            Some("string")
        );
        assert!(tool.extra.contains_key("x-extra"));
        assert!(parsed.extra.contains_key("meta"));

        let call_json = json!({
            "content": [
                {"type": "text", "text": "hello"}
            ],
            "isError": false,
            "structuredContent": {"kind": "object", "value": {"message": "ok"}},
            "requestId": "abc123"
        });

        let call: CallToolResult = serde_json::from_value(call_json).expect("parse call result");
        assert_eq!(call.is_error, Some(false));
        assert!(call.structured_content.is_some());
        assert!(call.extra.contains_key("requestId"));
        assert_eq!(call.content.len(), 1);
        assert_eq!(call.content[0].text.as_deref(), Some("hello"));
    }
}
