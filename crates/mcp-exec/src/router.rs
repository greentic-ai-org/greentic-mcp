use base64::Engine;
use serde_json::Value;
use wasmtime::component::Linker;

use crate::runner::StoreState;

mod bindings {
    wasmtime::component::bindgen!({
        path: "../mcp-adapter/wit/deps/wasix-mcp-25.6.18",
        world: "mcp-router",
    });
}

use bindings::McpRouter;
use bindings::exports::wasix::mcp::router::{ContentBlock, Response, ToolError, ToolResult};

pub fn try_call_tool_router(
    component: &wasmtime::component::Component,
    linker: &mut Linker<StoreState>,
    store: &mut wasmtime::Store<StoreState>,
    tool: &str,
    arguments_json: &String,
) -> anyhow::Result<Option<Value>> {
    let router = match McpRouter::instantiate(&mut *store, component, linker) {
        Ok(router) => router,
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("unknown export") || msg.contains("No such export") {
                return Ok(None);
            }
            return Err(err);
        }
    };

    let response = match router
        .wasix_mcp_router()
        .call_call_tool(&mut *store, tool, arguments_json)
    {
        Ok(Ok(resp)) => resp,
        Ok(Err(err)) => return Ok(Some(tool_error_to_value(tool, err))),
        Err(err) => return Err(err),
    };

    Ok(Some(render_response(&response)))
}

fn render_response(response: &Response) -> Value {
    match response {
        Response::Completed(result) => render_tool_result(result),
        Response::Elicit(req) => serde_json::json!({
            "ok": true,
            "elicitation": {
                "title": req.title.clone(),
                "message": req.message.clone(),
                "schema": req.schema.clone(),
            }
        }),
    }
}

fn render_tool_result(result: &ToolResult) -> Value {
    let (content, structured_content) = result.content.iter().map(render_content_block).fold(
        (Vec::new(), Vec::new()),
        |mut acc, (c, s)| {
            acc.0.push(c);
            if let Some(s) = s {
                acc.1.push(s);
            }
            acc
        },
    );

    serde_json::json!({
        "ok": true,
        "result": {
            "content": content,
            "structured_content": if structured_content.is_empty() { None } else { Some(structured_content) },
        }
    })
}

fn render_content_block(block: &ContentBlock) -> (Value, Option<Value>) {
    match block {
        ContentBlock::Text(text) => (serde_json::json!({"type": "text", "text": text.text}), None),
        ContentBlock::Image(img) => (
            serde_json::json!({"type": "image", "data": base64::engine::general_purpose::STANDARD.encode(&img.data), "mime_type": img.mime_type}),
            None,
        ),
        ContentBlock::ResourceLink(link) => (
            serde_json::json!({"type": "resource", "uri": link.uri.clone()}),
            None,
        ),
        ContentBlock::EmbeddedResource(res) => (
            serde_json::json!({"type": "resource-embed", "uri": res.uri.clone(), "data": base64::engine::general_purpose::STANDARD.encode(&res.data)}),
            None,
        ),
        ContentBlock::Audio(audio) => (
            serde_json::json!({"type": "audio", "data": base64::engine::general_purpose::STANDARD.encode(&audio.data), "mime_type": audio.mime_type}),
            None,
        ),
    }
}

fn tool_error_to_value(tool: &str, err: ToolError) -> Value {
    let (code, status, message) = match err {
        ToolError::InvalidParameters(msg) => ("MCP_TOOL_ERROR", 400, msg),
        ToolError::ExecutionError(msg) => ("MCP_TOOL_ERROR", 500, msg),
        ToolError::SchemaError(msg) => ("MCP_TOOL_ERROR", 422, msg),
        ToolError::NotFound(msg) => ("MCP_TOOL_ERROR", 404, msg),
    };

    serde_json::json!({
        "ok": false,
        "error": {
            "code": code,
            "message": message,
            "status": status,
            "tool": tool,
            "protocol": "25.06.18",
        }
    })
}
