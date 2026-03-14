pub mod tool;
pub mod builtin_tools;
pub mod web_search_tool;

use anyhow::Result;
use engine::{InferenceEngine, Message, Role};
use graph_db::GraphDB;
use log::{info, warn};
use regex::Regex;

pub struct Orchestrator {
    engine: InferenceEngine,
    graph: GraphDB,
    memory: Vec<Message>,
    tools: Vec<Box<dyn tool::Tool>>,
}

impl Orchestrator {
    pub fn new(engine: InferenceEngine, graph: GraphDB) -> Self {
        Self {
            engine,
            graph,
            memory: Vec::new(),
            tools: Vec::new(),
        }
    }

    pub fn register_tool(&mut self, tool: Box<dyn tool::Tool>) {
        self.tools.push(tool);
    }

    /// Dynamically generates the System Prompt based on registered MCP-style tools
    fn build_system_prompt(&self) -> String {
        let mut prompt = "You are Omniscient, an elite autonomous AI research agent based on the Claude-Task-Master architecture.\n".to_string();
        prompt.push_str("You operate in a continuous loop of reasoning and acting. You MUST use the tools provided to find actual data before answering.\n\n");

        prompt.push_str("AVAILABLE TOOLS:\n");
        for tool in &self.tools {
            prompt.push_str(&format!("- **{}**: {}\n  Schema: {}\n\n",
                tool.name(),
                tool.description(),
                tool.parameters_schema().to_string()
            ));
        }

        prompt.push_str(r#"
To use a tool, you MUST output EXACTLY:
<action>
{"tool": "tool_name", "args": {"param": "value"}}
</action>

To finish the task and provide the final result to the user, output:
<action>
{"tool": "final_answer", "args": {"content": "Your final detailed summary here"}}
</action>

Do NOT invent information. If you don't know, use a tool to search.
"#);
        prompt
    }

    /// The STORM-style Planner Phase
    async fn generate_plan(&mut self, query: &str) -> Result<String> {
        info!("--- Planner Phase ---");
        let planner_prompt = format!("Create a concise 3-step research plan to solve this task: '{}'. Only output the numbered steps.", query);

        let plan_memory = vec![
            Message { role: Role::System, content: "You are a master research planner.".to_string() },
            Message { role: Role::User, content: planner_prompt }
        ];

        let plan = self.engine.generate(&plan_memory, 200)?;
        info!("Generated Plan:\n{}", plan);
        Ok(plan)
    }

    /// Initializes a research task using a strict ReAct (Reason + Act) loop structure.
    pub async fn execute_research(&mut self, query: &str) -> Result<String> {
        info!("Starting ReAct orchestrator for query: {}", query);

        let plan = self.generate_plan(query).await?;

        let system_prompt = self.build_system_prompt();

        self.memory.push(Message { role: Role::System, content: system_prompt });
        self.memory.push(Message {
            role: Role::User,
            content: format!("Task: {}\n\nYour Execution Plan:\n{}", query, plan)
        });

        // Expanded max iterations for complex multi-tool workflows
        let max_iterations = 15;
        let action_regex = Regex::new(r"(?s)<action>(.*?)</action>")?;

        for step in 1..=max_iterations {
            info!("--- ReAct Step {} ---", step);

            // 1. Generate Thought and Action
            // We give it slightly more tokens to explain its reasoning before outputting <action>
            let response = self.engine.generate(&self.memory, 300)?;
            info!("Agent Output: \n{}", response);
            self.memory.push(Message { role: Role::Assistant, content: response.clone() });

            // 2. Parse Actions
            if let Some(captures) = action_regex.captures(&response) {
                let json_str = captures.get(1).map_or("", |m| m.as_str()).trim();

                let action: serde_json::Value = match serde_json::from_str(json_str) {
                    Ok(val) => val,
                    Err(e) => {
                        let err_msg = format!("JSON Parse Error: {}. Ensure you use double quotes.", e);
                        warn!("{}", err_msg);
                        self.memory.push(Message { role: Role::Tool, content: err_msg });
                        continue;
                    }
                };

                let tool_name = action.get("tool").and_then(|t| t.as_str()).unwrap_or("unknown");
                let default_args = serde_json::json!({});
                let args = action.get("args").unwrap_or(&default_args);

                if tool_name == "final_answer" {
                    let final_content = args.get("content").and_then(|c| c.as_str()).unwrap_or("Done.");
                    info!("Agent declared final answer reached.");

                    // Persist to graph
                    let props = serde_json::json!({"query": query, "answer_length": final_content.len()});
                    let _ = self.graph.insert_node("FinalResearch", &props);

                    return Ok(final_content.to_string());
                }

                // Dynamic Tool Routing
                let mut executed = false;
                for tool in &self.tools {
                    if tool.name() == tool_name {
                        info!("Executing Tool: {}", tool_name);
                        match tool.execute(args).await {
                            Ok(result) => {
                                // Cap result size so we don't blow out the 4GB RAM LLM context window (approx 2000 chars)
                                let truncated = if result.len() > 2000 {
                                    format!("{}... [TRUNCATED]", &result[0..2000])
                                } else {
                                    result
                                };
                                self.memory.push(Message { role: Role::Tool, content: format!("Tool Result:\n{}", truncated) });
                            }
                            Err(e) => {
                                warn!("Tool {} failed: {}", tool_name, e);
                                self.memory.push(Message { role: Role::Tool, content: format!("Tool Error: {}", e) });
                            }
                        }
                        executed = true;
                        break;
                    }
                }

                if !executed {
                    self.memory.push(Message { role: Role::Tool, content: format!("Unknown tool '{}'", tool_name) });
                }
            } else {
                warn!("No <action> block found. Reminding agent to use tools.");
                self.memory.push(Message { role: Role::Tool, content: "You did not output an <action> JSON block. Please use the tools to continue or output final_answer.".to_string() });
            }
        }

        Ok("Research aborted: Exceeded maximum iterations.".to_string())
    }
}
