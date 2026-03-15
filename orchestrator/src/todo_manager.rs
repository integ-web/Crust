use std::collections::VecDeque;

/// To prevent the agent from "drifting" (s03), a TodoManager requires the agent
/// to list its intended steps before execution, a practice that doubles completion rates.
pub struct TodoManager {
    queue: VecDeque<String>,
    completed: Vec<String>,
}

impl TodoManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            completed: Vec::new(),
        }
    }

    pub fn commit_step(&mut self, step: &str) {
        self.queue.push_back(step.to_string());
    }

    pub fn next_step(&mut self) -> Option<String> {
        self.queue.pop_front()
    }

    pub fn mark_completed(&mut self, step: &str) {
        self.completed.push(step.to_string());
    }

    pub fn remaining_steps(&self) -> usize {
        self.queue.len()
    }
}
