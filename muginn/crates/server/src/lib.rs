//! Muginn MCP server: pure handlers (`handlers`) + a thin rmcp stdio binding.

pub mod handlers;

use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::{tool, ServerHandler};
use std::sync::{Arc, Mutex};

use muginn_store::Store;

/// Shared server state. The Store is behind a Mutex because rmcp handlers take `&self`
/// and rusqlite's Connection is not Sync.
#[derive(Clone)]
pub struct MuginnServer {
    store: Arc<Mutex<Store>>,
    priv_hex: String,
    pub_hex: String,
}

impl MuginnServer {
    pub fn new(db_path: &str, priv_hex: String, pub_hex: String) -> Self {
        let store = Store::open(db_path);
        MuginnServer {
            store: Arc::new(Mutex::new(store)),
            priv_hex,
            pub_hex,
        }
    }
}

#[tool(tool_box)]
impl MuginnServer {
    #[tool(description = "Recall compiled cards + grounded atoms for a query, each with a verify status.")]
    async fn recall(
        &self,
        #[tool(param)] query: String,
        #[tool(param)] k: Option<u32>,
    ) -> Result<CallToolResult, rmcp::Error> {
        let store = self.store.lock().unwrap();
        let out = handlers::recall(&store, &query, k.unwrap_or(5) as usize);
        Ok(CallToolResult::success(vec![Content::text(out)]))
    }

    #[tool(description = "Verify an atom by id: re-open source, byte-compare. Returns a status string.")]
    async fn verify(&self, #[tool(param)] atom_id: String) -> Result<CallToolResult, rmcp::Error> {
        let store = self.store.lock().unwrap();
        let status = handlers::verify(&store, &atom_id);
        Ok(CallToolResult::success(vec![Content::text(status)]))
    }

    #[tool(description = "Return the exact citation JSON {agent, session, turn, span} for click-through.")]
    async fn cite(&self, #[tool(param)] atom_id: String) -> Result<CallToolResult, rmcp::Error> {
        let store = self.store.lock().unwrap();
        let json = handlers::cite(&store, &atom_id);
        Ok(CallToolResult::success(vec![Content::text(json.to_string())]))
    }

    #[tool(description = "Ingest a transcript: run adapter, select salient spans, verified-store atoms.")]
    async fn ingest(
        &self,
        #[tool(param)] agent: String,
        #[tool(param)] path: String,
    ) -> Result<CallToolResult, rmcp::Error> {
        let store = self.store.lock().unwrap();
        let n = handlers::ingest(&store, &agent, &path, &self.priv_hex, &self.pub_hex);
        Ok(CallToolResult::success(vec![Content::text(format!("ingested {n} atoms"))]))
    }

    #[tool(description = "Compile a page for a topic with citation enforcement; reports coverage.")]
    async fn compile(&self, #[tool(param)] topic: String) -> Result<CallToolResult, rmcp::Error> {
        let store = self.store.lock().unwrap();
        let out = handlers::compile(&store, &topic);
        Ok(CallToolResult::success(vec![Content::text(out)]))
    }
}

#[tool(tool_box)]
impl ServerHandler for MuginnServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Muginn — verifiable memory. Tools: recall, verify, cite, ingest, compile."
                    .to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

/// Serve over stdio (blocking until the client disconnects).
pub async fn serve_stdio(server: MuginnServer) -> anyhow::Result<()> {
    use rmcp::ServiceExt;
    let transport = rmcp::transport::io::stdio();
    let running = server.serve(transport).await?;
    running.waiting().await?;
    Ok(())
}
