mod server;

use std::sync::Arc;

use lxs_runtime::Runtime;
use server::McpServer;

#[tokio::main]
async fn main() {
    let runtime = Arc::new(Runtime::new());
    let server = McpServer::new(runtime);
    server.run_stdio().await.unwrap();
}
