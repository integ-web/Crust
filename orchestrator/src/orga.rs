use anyhow::Result;
use brain::taint::{PrincipalChecker, UntrustedValue, TrustedAction};
use crate::todo_manager::TodoManager;
use tracing::info;

pub struct OrgaCycle {
    todo_manager: TodoManager,
}

impl OrgaCycle {
    pub fn new() -> Self {
        Self {
            todo_manager: TodoManager::new(),
        }
    }

    /// The Observe-Reason-Gate-Act (ORGA) cycle.
    /// This represents the core agent loop.
    pub async fn run_cycle(&mut self, goal: &str) -> Result<()> {
        info!("=== Starting ORGA Cycle for Goal: {} ===", goal);

        // 1. Observe (Extract context / Generate Plan)
        // In a real system, the LLM sets the steps. For now, we mock.
        self.todo_manager.commit_step("Scrape user input");
        self.todo_manager.commit_step("Process and write summary to disk");

        while let Some(step) = self.todo_manager.next_step() {
            info!("ORGA Step: {}", step);

            // 2. Reason (Agent determines action)
            // Mock: We scraped some data from a "web source".
            let scraped_data = "summary of personal finance tools";
            let untrusted = UntrustedValue::new(scraped_data.to_string(), "web".to_string());

            // 3. Gate (Principal Checker Enforces Policy P-T)
            // The action is to write to disk. We must sanitize the input first.
            let policy = |data: &String| !data.contains("rm -rf"); // Simple mock policy

            let trusted_result = PrincipalChecker::sanitize(untrusted, policy);

            match trusted_result {
                Ok(trusted_value) => {
                    info!("Gate Passed: Data is trusted.");

                    // 4. Act
                    // We execute a mock TrustedAction that strictly requires TrustedValue.
                    let action = MockFileSystemAction;
                    let success = action.execute(trusted_value);
                    if success {
                        info!("Action Executed Successfully.");
                    }
                }
                Err(e) => {
                    tracing::error!("Gate Failed: Security Policy Violation! -> {}", e);
                    anyhow::bail!("ORGA Cycle Aborted due to security violation.");
                }
            }

            self.todo_manager.mark_completed(&step);
        }

        info!("=== ORGA Cycle Completed Successfully ===");
        Ok(())
    }
}

// Mock Action for the Orchestrator
struct MockFileSystemAction;

impl TrustedAction for MockFileSystemAction {
    type Input = String;
    type Output = bool;

    fn execute(&self, input: brain::taint::TrustedValue<Self::Input>) -> Self::Output {
        let safe_data = input.into_inner();
        info!("Writing safe data to disk: {}", safe_data);
        true
    }
}
