use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Tool: Send + Sync {
    /// The exact name the LLM must use to call this tool (e.g., "search_web")
    fn name(&self) -> &'static str;

    /// A description injected into the system prompt explaining when and how to use it
    fn description(&self) -> &'static str;

    /// The JSON schema representing the arguments this tool requires
    fn parameters_schema(&self) -> serde_json::Value;

    /// Execute the tool with the provided parsed JSON arguments
    async fn execute(&self, args: &serde_json::Value) -> Result<String>;
}
