use std::sync::Arc;
use tokio::sync::{mpsc};

use crate::common_types::{AgentConfig};
use crate::communication_bus::BusRequest;
use crate::common_types::agent_defs::{AgentStatus, AgentCapabilities}; // Import AgentStatus and AgentCapabilities
use crate::mcp_manager::MCPManager;

pub struct BaseAgentComponents {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub capabilities: AgentCapabilities,
    pub config: AgentConfig,
    pub bus_sender: mpsc::Sender<BusRequest>,
    pub mcp_manager: Arc<MCPManager>,
    pub current_task_id: Option<uuid::Uuid>,
    pub current_task: Option<crate::common_types::task_graph_defs::TaskNode>,
    pub received_information: Option<crate::common_types::message_defs::InformationResponse>,
}

impl BaseAgentComponents {
    pub fn new(
        id: String,
        name: String,
        status: AgentStatus,
        capabilities: AgentCapabilities,
        mcp_manager: Arc<MCPManager>,
        config: AgentConfig,
        bus_request_sender: mpsc::Sender<BusRequest>, // Use mpsc::Sender for bus requests
    ) -> Self {
        BaseAgentComponents {
            id,
            name,
            status,
            capabilities,
            config,
            bus_sender: bus_request_sender, // Map bus_request_sender to bus_sender
            mcp_manager,
            current_task_id: None, // Added field
            current_task: None, // Added field
            received_information: None, // Added field
        }
    }

    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_status(&self) -> AgentStatus {
        self.status.clone() // Return a clone
    }

    pub fn set_status(&mut self, status: AgentStatus) {
        self.status = status;
    }

    pub fn get_capabilities(&self) -> AgentCapabilities {
        self.capabilities.clone()
    }

    pub fn get_config(&self) -> AgentConfig {
        self.config.clone()
    }
}