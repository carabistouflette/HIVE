use crate::common_types::Message;
use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc};
use crate::common_types::agent_defs::{AgentCapabilities, AgentStatus};
use crate::common_types::task_graph_defs::TaskNode;
use crate::common_types::message_defs::MessageContent;
use crate::communication_bus::BusRequest; // Import BusRequest
use std::sync::Arc; // Import Arc

#[async_trait]
pub trait Agent: Send + Sync {
    async fn id(&self) -> String;
    async fn name(&self) -> String;
    async fn get_status(&self) -> AgentStatus;
    async fn set_status(&self, status: AgentStatus);
    async fn get_capabilities(&self) -> AgentCapabilities;
    async fn get_config(&self) -> crate::AgentConfig;
    async fn start(self: Arc<Self>, bus_receiver: broadcast::Receiver<Message>, bus_sender: mpsc::Sender<BusRequest>);
    async fn process_task(&self, task: TaskNode) -> anyhow::Result<()>;
}