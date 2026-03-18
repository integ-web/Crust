use anyhow::Result;
use brain::taint::{PrincipalChecker, UntrustedValue, TrustedAction};
use crate::todo_manager::TodoManager;
use tracing::info;
use petgraph::graph::DiGraph;
use petgraph::algo::toposort;
use tokio::task::JoinSet;
use std::collections::HashMap;

/// A node in our research DAG
#[derive(Debug, Clone)]
pub struct ResearchTask {
    pub id: String,
    pub description: String,
    pub is_gate: bool,
}

pub struct OrgaCycle {
    pub todo_manager: TodoManager,
    pub dag: DiGraph<ResearchTask, ()>,
}

impl OrgaCycle {
    pub fn new() -> Self {
        Self {
            todo_manager: TodoManager::new(),
            dag: DiGraph::new(),
        }
    }

    /// The Observe-Reason-Gate-Act (ORGA) cycle via a true DAG execution model.
    pub async fn run_dag_cycle(&mut self, goal: &str) -> Result<()> {
        info!("=== Starting DAG ORGA Cycle for Goal: {} ===", goal);

        // 1. Observe (Plan Generation)
        let t1 = self.dag.add_node(ResearchTask {
            id: "T1".into(),
            description: "Scrape user input (Worker A)".into(),
            is_gate: false,
        });

        let t2 = self.dag.add_node(ResearchTask {
            id: "T2".into(),
            description: "Scrape secondary source (Worker B)".into(),
            is_gate: false,
        });

        let t3 = self.dag.add_node(ResearchTask {
            id: "T3".into(),
            description: "Process and write summary to disk (Requires T1 & T2)".into(),
            is_gate: true, // Requires PrincipalChecker
        });

        // T1 and T2 must happen before T3
        self.dag.add_edge(t1, t3, ());
        self.dag.add_edge(t2, t3, ());

        // Get topological sort to determine execution order layers
        let sorted_indices = match toposort(&self.dag, None) {
            Ok(s) => s,
            Err(_) => {
                anyhow::bail!("Cycle detected in Research DAG! Aborting to prevent infinite loop.");
            }
        };

        // Group tasks by their dependencies to allow true parallel execution.
        // For simplicity in this mock, we just process them in sorted order but spawn them.
        for node_idx in sorted_indices {
            let task = self.dag[node_idx].clone();
            info!("Spawning DAG Task [{}]: {}", task.id, task.description);
            self.todo_manager.commit_step(&task.id);

            // 2. Reason & 4. Act
            if task.is_gate {
                // 3. Gate (Principal Checker Enforces Policy P-T)
                let scraped_data = "summary of personal finance tools";
                let untrusted = UntrustedValue::new(scraped_data.to_string(), "web".to_string());
                let policy = |data: &String| !data.contains("rm -rf");

                match PrincipalChecker::sanitize(untrusted, policy) {
                    Ok(trusted_value) => {
                        info!("Gate Passed for [{}]. Data is trusted.", task.id);
                        let action = MockFileSystemAction;
                        let _success = action.execute(trusted_value);
                    }
                    Err(e) => {
                        tracing::error!("Gate Failed for [{}]: Security Policy Violation! -> {}", task.id, e);
                        anyhow::bail!("ORGA Cycle Aborted due to security violation.");
                    }
                }
            } else {
                // Simulate parallel async work for non-gated tasks
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                info!("Task [{}] completed successfully.", task.id);
            }

            self.todo_manager.mark_completed(&task.id);
        }

        info!("=== DAG ORGA Cycle Completed Successfully ===");
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
