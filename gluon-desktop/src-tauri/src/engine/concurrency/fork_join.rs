// Fork-Join Coordination (stub - Phase 2)

use async_trait::async_trait;
use crate::engine::graph::{Node, NodeType};
use crate::engine::execution::{ControlFlow, ExecutionContext};
use crate::engine::EngineResult;

pub struct ForkNode {
    pub id: String,
    pub name: String,
}

impl ForkNode {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

#[async_trait]
impl Node for ForkNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Fork
    }

    async fn execute(&self, _context: &mut ExecutionContext) -> EngineResult<ControlFlow> {
        // Stub - Phase 2
        Ok(ControlFlow::Continue)
    }
}

pub struct JoinNode {
    pub id: String,
    pub name: String,
}

impl JoinNode {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

#[async_trait]
impl Node for JoinNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Join
    }

    async fn execute(&self, _context: &mut ExecutionContext) -> EngineResult<ControlFlow> {
        // Stub - Phase 2
        Ok(ControlFlow::Continue)
    }
}
