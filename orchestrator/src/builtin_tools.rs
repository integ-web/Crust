use anyhow::Result;
use async_trait::async_trait;
use scraper::ToolSynthesizer;
use std::sync::Arc;
use crate::tool::Tool;

pub struct ArxivTool {
    scraper: Arc<ToolSynthesizer>,
}

impl ArxivTool {
    pub fn new(scraper: Arc<ToolSynthesizer>) -> Self {
        Self { scraper }
    }
}

#[async_trait]
impl Tool for ArxivTool {
    fn name(&self) -> &'static str {
        "arxiv"
    }

    fn description(&self) -> &'static str {
        "Search the Arxiv database for scientific and academic papers."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query, e.g., 'machine learning'."
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
        let query_clean = query.replace(" ", "+");
        self.scraper.search_arxiv(&query_clean).await
    }
}

pub struct BrowseTool {
    scraper: Arc<ToolSynthesizer>,
}

impl BrowseTool {
    pub fn new(scraper: Arc<ToolSynthesizer>) -> Self {
        Self { scraper }
    }
}

#[async_trait]
impl Tool for BrowseTool {
    fn name(&self) -> &'static str {
        "browse"
    }

    fn description(&self) -> &'static str {
        "Launch a headless browser to visit a URL and extract its text content."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The full HTTPS URL to browse."
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("");
        self.scraper.browse_and_extract(url).await
    }
}
