use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use greentic_mcp::compose::compose_router_with_bundled_adapter;

#[derive(Parser)]
#[command(
    name = "greentic-mcp",
    version,
    about = "Compose MCP router components with the bundled adapter"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compose a router component into the bundled adapter.
    Compose(ComposeArgs),
}

#[derive(Parser)]
struct ComposeArgs {
    /// Path to a wasix:mcp router component (.wasm).
    #[arg(value_name = "ROUTER_WASM")]
    router: PathBuf,
    /// Path to write the composed component.
    #[arg(short, long, value_name = "OUTPUT_WASM")]
    output: PathBuf,
    /// Path to wasm-tools (defaults to GREENTIC_MCP_WASM_TOOLS or wasm-tools in PATH).
    #[arg(long, value_name = "PATH")]
    wasm_tools: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compose(args) => compose_router_with_bundled_adapter(
            &args.router,
            &args.output,
            args.wasm_tools.as_deref(),
        ),
    }
}
