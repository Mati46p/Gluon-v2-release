// Checkpoint - Execution state snapshot (stub - to be implemented in Phase 3)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub execution_id: String,
    pub graph_id: String,
    pub current_node: String,
    pub blackboard_json: String,
    pub history_json: String,
    pub budget_json: String,
    pub created_at: i64,
}

impl Checkpoint {
    pub fn new(execution_id: String, graph_id: String) -> Self {
        Self {
            execution_id,
            graph_id,
            current_node: String::new(),
            blackboard_json: String::new(),
            history_json: String::new(),
            budget_json: String::new(),
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}
