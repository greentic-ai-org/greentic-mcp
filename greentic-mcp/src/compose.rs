use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ADAPTER_COMPONENT: &[u8] = include_bytes!("../assets/mcp_adapter_25_06_18.component.wasm");

pub const ADAPTER_PROTOCOL: &str = "25.06.18";

pub fn compose_router_with_bundled_adapter(
    router: &Path,
    output: &Path,
    wasm_tools: Option<&Path>,
) -> Result<()> {
    if !router.exists() {
        return Err(anyhow!("router component not found: {}", router.display()));
    }

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating output directory {}", parent.display()))?;
    }

    let wasm_tools = resolve_wasm_tools(wasm_tools)?;
    let adapter_path = write_adapter_component()?;

    let output = output.to_path_buf();
    let status = Command::new(&wasm_tools)
        .arg("compose")
        .arg(adapter_path.path())
        .arg("-d")
        .arg(router)
        .arg("-o")
        .arg(&output)
        .status()
        .with_context(|| format!("running {}", wasm_tools.display()))?;

    if !status.success() {
        return Err(anyhow!("wasm-tools compose failed with status {status}"));
    }

    Ok(())
}

fn resolve_wasm_tools(wasm_tools: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = wasm_tools {
        return Ok(path.to_path_buf());
    }
    if let Ok(path) = std::env::var("GREENTIC_MCP_WASM_TOOLS")
        && !path.trim().is_empty()
    {
        return Ok(PathBuf::from(path));
    }
    Ok(PathBuf::from("wasm-tools"))
}

fn write_adapter_component() -> Result<tempfile::NamedTempFile> {
    let mut file = tempfile::Builder::new()
        .prefix("mcp_adapter_")
        .suffix(".component.wasm")
        .tempfile()
        .context("creating temp adapter component")?;
    std::io::Write::write_all(&mut file, ADAPTER_COMPONENT)
        .context("writing bundled adapter component")?;
    Ok(file)
}
