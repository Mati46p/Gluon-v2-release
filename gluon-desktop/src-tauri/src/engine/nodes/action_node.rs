// Action Node - Shell/File operations (stub - Phase 2+)

use async_trait::async_trait;
use crate::engine::graph::{Node, NodeType};
use crate::engine::execution::{ControlFlow, ExecutionContext};
use crate::engine::EngineResult;

pub struct ActionNode {
    pub id: String,
    pub name: String,
}

impl ActionNode {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

#[async_trait]
impl Node for ActionNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Action
    }

    async fn execute(&self, _context: &mut ExecutionContext) -> EngineResult<ControlFlow> {
        // Stub - to be implemented
        Ok(ControlFlow::Continue)
    }
}
