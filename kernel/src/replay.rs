use anyhow::Result;
use crate::event_log::EventLog;
use serde_json::Value;

/// The Replay Engine ensures crash-resilience.
/// When the agent starts, it checks the log. If a task was interrupted,
/// it "replays" the previous results into the agent's context instead of re-executing.
pub struct ReplayEngine {
    pub log: EventLog,
}

impl ReplayEngine {
    pub fn new(db_path: &str) -> Result<Self> {
        let log = EventLog::new(db_path)?;
        Ok(Self { log })
    }

    /// Attempts to replay an action. If it exists in the log, returns the result immediately.
    /// If not, it executes the provided closure (which represents the real external effect),
    /// records the outcome in the log, and then returns the result.
    pub async fn execute_or_replay<F, Fut>(
        &self,
        tool_name: &str,
        arguments: Value,
        actual_execution: F,
    ) -> Result<Value>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Value>>,
    {
        // 1. Check if the event was already recorded (crash resilience)
        if let Some(event) = self.log.get_event(tool_name, &arguments)? {
            tracing::info!("Replay Engine: Replaying previously executed tool `{}` with args `{:?}`", tool_name, arguments);
            return Ok(event.result);
        }

        // 2. If not found, execute the actual logic
        tracing::info!("Replay Engine: Executing tool `{}` for the first time", tool_name);
        let result = actual_execution().await?;

        // 3. Record the result for future replays
        self.log.record_event(tool_name, &arguments, &result)?;

        Ok(result)
    }
}
