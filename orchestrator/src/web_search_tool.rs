use anyhow::Result;
use async_trait::async_trait;
use scraper::ToolSynthesizer;
use std::sync::Arc;
use crate::tool::Tool;

pub struct WebSearchTool {
    scraper: Arc<ToolSynthesizer>,
}

impl WebSearchTool {
    pub fn new(scraper: Arc<ToolSynthesizer>) -> Self {
        Self { scraper }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "search_web"
    }

    fn description(&self) -> &'static str {
        "Search the internet (via DuckDuckGo) for real-time information, companies, events, or general knowledge. Returns a list of URLs and snippets."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The exact search query to look up on the web."
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
        self.scraper.search_web(query).await
    }
}
