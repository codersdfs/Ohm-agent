use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TableQuery {
    pub path: String,
    pub query: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

pub async fn query_table(request: TableQuery) -> Result<serde_json::Value, String> {
    log::info!("query_table: path={}", request.path);
    Ok(serde_json::json!({"status": "not_implemented"}))
}
