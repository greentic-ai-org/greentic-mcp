//! Configuration primitives describing how the executor resolves, verifies, and
//! runs Wasm components.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use greentic_types::TenantCtx;

use crate::store::ToolStore;

/// Configuration for a single executor invocation.
#[derive(Clone)]
pub struct ExecConfig {
    pub store: ToolStore,
    pub security: VerifyPolicy,
    pub runtime: RuntimePolicy,
    pub http_enabled: bool,
    /// Optional secrets-store binding implementing greentic:secrets/store@1.0.0.
    /// When absent, secrets imports will return a host error.
    pub secrets_store: Option<DynSecretsStore>,
}

/// Policy describing how artifacts must be verified prior to execution.
#[derive(Clone, Debug, Default)]
pub struct VerifyPolicy {
    /// Whether artifacts without a matching digest/signature are still allowed.
    pub allow_unverified: bool,
    /// Expected digests (hex encoded) keyed by component identifier.
    pub required_digests: HashMap<String, String>,
    /// Signers that are trusted to vouch for artifacts.
    pub trusted_signers: Vec<String>,
}

/// Runtime resource limits applied to the Wasm execution.
#[derive(Clone, Debug)]
pub struct RuntimePolicy {
    pub fuel: Option<u64>,
    pub max_memory: Option<u64>,
    pub wallclock_timeout: Duration,
    pub per_call_timeout: Duration,
    pub max_attempts: u32,
    pub base_backoff: Duration,
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        Self {
            fuel: None,
            max_memory: None,
            wallclock_timeout: Duration::from_secs(30),
            per_call_timeout: Duration::from_secs(10),
            max_attempts: 1,
            base_backoff: Duration::from_millis(100),
        }
    }
}

/// Host-facing secrets-store trait mirroring greentic:secrets/store@1.0.0.
pub trait SecretsStore: Send + Sync {
    /// Read bytes for the scoped secret name.
    fn read(&self, scope: &TenantCtx, name: &str) -> Result<Vec<u8>, String>;

    /// Upsert bytes for the scoped secret name. Defaults to an error when not implemented.
    fn write(&self, scope: &TenantCtx, name: &str, bytes: &[u8]) -> Result<(), String> {
        let _ = (scope, name, bytes);
        Err("write-not-implemented".into())
    }

    /// Delete the scoped secret. Defaults to an error when not implemented.
    fn delete(&self, scope: &TenantCtx, name: &str) -> Result<(), String> {
        let _ = (scope, name);
        Err("delete-not-implemented".into())
    }
}

/// Shared secrets-store handle.
pub type DynSecretsStore = Arc<dyn SecretsStore>;

impl fmt::Debug for ExecConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecConfig")
            .field("store", &self.store)
            .field("security", &self.security)
            .field("runtime", &self.runtime)
            .field("http_enabled", &self.http_enabled)
            .field(
                "secrets_store",
                &self.secrets_store.as_ref().map(|_| "<dyn SecretsStore>"),
            )
            .finish()
    }
}
