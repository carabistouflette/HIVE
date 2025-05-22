use async_trait::async_trait;
use tokio::sync::{mpsc, broadcast, Mutex};
use tracing::{info, error};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;

use crate::agents::base_agent::{Agent};
use crate::agents::base_agent_components::BaseAgentComponents;
use crate::common_types::agent_defs::{AgentStatus, AgentCapabilities};
use crate::common_types::task_graph_defs::TaskNode;
use crate::common_types::sprint_defs::Deliverable;
use crate::common_types::message_defs::{Message, MessageContent, InformationRequest, AgentResponse}; // Added AgentResponse
use crate::common_types::mcp_defs::{MCPInput};
use crate::communication_bus::BusRequest;
use crate::mcp_manager::MCPManager;
use crate::AgentConfig;

pub struct WriterAgent {
    pub components: Mutex<BaseAgentComponents>,
}

impl WriterAgent {
    pub async fn new(
        id: String,
        name: String,
        config: AgentConfig,
        mcp_manager: Arc<MCPManager>,
        bus_request_sender: mpsc::Sender<BusRequest>,
    ) -> Result<Self, anyhow::Error> {
        let components = Mutex::new(BaseAgentComponents::new(
            id,
            name,
            AgentStatus::Idle,
            AgentCapabilities {
                can_research: false,
                can_write: true,
                can_plan: false,
                can_code: false,
                can_design: false,
                can_test: false,
                can_debug: false,
                can_architect: false,
                can_manage_sprint: false,
                can_use_tool: false,
            },
            mcp_manager,
            config,
            bus_request_sender,
        ));
        Ok(Self { components })
    }
}

#[async_trait]
impl Agent for WriterAgent {
    async fn id(&self) -> String {
        let components = self.components.lock().await;
        components.get_id()
    }

    async fn name(&self) -> String {
        let components = self.components.lock().await;
        components.get_name()
    }

    async fn get_status(&self) -> AgentStatus {
        let components = self.components.lock().await;
        components.get_status()
    }

    async fn set_status(&self, status: AgentStatus) {
        let mut components = self.components.lock().await;
        components.set_status(status);
    }

    async fn get_capabilities(&self) -> AgentCapabilities {
        let components = self.components.lock().await;
        components.get_capabilities()
    }

    async fn get_config(&self) -> crate::AgentConfig {
        let components = self.components.lock().await;
        components.config.clone()
    }

    async fn start(self: Arc<Self>, mut bus_receiver: broadcast::Receiver<Message>, bus_sender: mpsc::Sender<BusRequest>) {
        info!("WriterAgent {} started", self.id().await);
        loop {
            tokio::select! {
                result = bus_receiver.recv() => { // Only one recv() call
                    match result {
                        Ok(message) => {
                            info!("WriterAgent {} received message in start loop: {:?}", self.id().await, message);
                            match message.content {
                                MessageContent::TaskAssignment { task } => {
                                    // Handle new task assignment
                                    self.set_status(AgentStatus::Busy).await;
                                    let agent_id = self.id().await;
                                    let task_id = task.id.clone();
                                    // No need to clone self here, as process_task is called directly
                                    if let Err(e) = self.process_task(task).await {
                                        error!("Error processing task for WriterAgent {}: {}", agent_id, e);
                                        let response = AgentResponse::TaskFailed {
                                            task_id: task_id.to_string(),
                                            agent_id: agent_id.clone(),
                                            error: format!("Task processing failed: {}", e),
                                        };
                                        let components = self.components.lock().await;
                                        if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                                            error!("Failed to send TaskFailed response from WriterAgent: {}", e);
                                        }
                                    }
                                },
                                MessageContent::ReturnInformation(info_response) => {
                                    // Handle returned information
                                    let mut components = self.components.lock().await;
                                    if components.get_status() == AgentStatus::WaitingForInformation && components.current_task_id.as_ref().map(|id| id.to_string()) == Some(info_response.original_task_id.clone()) { // Cloned original_task_id
                                        info!("WriterAgent {} received ReturnInformation for task {}", components.id, info_response.original_task_id);
                                        components.received_information = Some(info_response);
                                        components.set_status(AgentStatus::Busy);
                                        if let Some(task) = components.current_task.take() {
                                            let agent_id = components.id.clone();
                                            let task_id = task.id.clone();
                                            drop(components); // Release the lock before calling process_task
                                            // No need to clone self here, as process_task is called directly
                                            if let Err(e) = self.process_task(task).await {
                                                error!("Error processing task after receiving information for WriterAgent {}: {}", agent_id, e);
                                                let response = AgentResponse::TaskFailed {
                                                    task_id: task_id.to_string(),
                                                    agent_id,
                                                    error: format!("Task processing failed after research: {}", e),
                                                };
                                                let components = self.components.lock().await;
                                                if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                                                    error!("Failed to send TaskFailed response from WriterAgent: {}", e);
                                                }
                                            }
                                        } else {
                                            error!("WriterAgent {} received ReturnInformation but no current task was stored.", components.id);
                                            components.set_status(AgentStatus::Idle);
                                        }
                                    } else {
                                        info!("WriterAgent {} received ReturnInformation for a different task or not in WaitingForInformation state. Ignoring.", components.id);
                                    }
                                },
                                _ => {
                                    // Ignore other message types for now
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            error!("WriterAgent {} lagged behind by {} messages. Skipping some messages.", self.id().await, n);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            info!("WriterAgent {} channel closed. Shutting down.", self.id().await);
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn process_task(&self, task: TaskNode) -> anyhow::Result<()> {
        let mut components = self.components.lock().await;
        info!("WriterAgent received task: {:?}", task.description);

        components.current_task_id = Some(task.id.clone());
        components.current_task = Some(task.clone());

        // Check if information is needed (Simplified logic)
        let needs_research = task.task_spec.description.to_lowercase().contains("needs research on");

        if needs_research && components.received_information.is_none() {
            info!("WriterAgent {} needs research for task {}", components.id, task.id);
            let info_request = InformationRequest {
                original_task_id: task.id.to_string(),
                requesting_agent_id: components.id.clone(),
                query: task.task_spec.description.clone(),
                context: task.task_spec.context.clone(),
            };
            let message = Message {
                id: Uuid::new_v4().to_string(),
                sender_id: components.id.clone(),
                receiver_id: None,
                content: MessageContent::RequestInformation(info_request),
            };
            components.set_status(AgentStatus::WaitingForInformation);
            components.bus_sender.send(BusRequest::GeneralMessage { message }).await?;
            return Ok(());
        }

        // If information was received (or not needed), proceed with MCP invocation
        let topic = task.task_spec.description.clone();
        let mut key_points: Option<Vec<String>> = None;
        let mut style_guide: Option<String> = None;
        let mut research_context: Option<String> = None;

        if let Some(details_str) = &task.task_spec.context {
            match serde_json::from_str::<Value>(details_str) {
                Ok(details_json) => {
                    if let Some(points) = details_json.get("key_points").and_then(|v| v.as_array()) {
                        key_points = Some(points.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect());
                    }
                    if let Some(style) = details_json.get("style_guide").and_then(|v| v.as_str()) {
                        style_guide = Some(style.to_string());
                    }
                }
                Err(e) => {
                    error!("WriterAgent failed to parse task details JSON: {}", e);
                }
            }
        }

        // Include received information in the MCP input
        if let Some(info_response) = components.received_information.take() {
            if info_response.error.is_none() {
                research_context = Some(info_response.response_data);
            } else {
                error!("WriterAgent {} received error in ReturnInformation: {}", components.id, info_response.error.unwrap());
            }
        }

        let mut mcp_input_data = serde_json::json!({
            "topic": topic,
        });

        if let Some(points) = key_points {
            mcp_input_data["key_points"] = serde_json::json!(points);
        }
        if let Some(style) = style_guide {
            mcp_input_data["style_guide"] = serde_json::json!(style);
        }
        if let Some(context) = research_context {
             mcp_input_data["research_context"] = serde_json::json!(context);
        }

        let mcp_input = MCPInput {
            mcp_id: "draft_content_v1".to_string(),
            data: mcp_input_data,
            context_overrides: None,
        };

        #[derive(Deserialize, Debug)]
        struct WritingMcpOutput {
            draft_text: String,
        }

        match components.mcp_manager.invoke_mcp(
            mcp_input,
        ).await {
            Ok(mcp_output) => {
                match serde_json::from_value::<WritingMcpOutput>(mcp_output.processed_content.unwrap_or_default()) {
                    Ok(parsed_output) => {
                        let deliverable = Deliverable::CodePatch { // Using CodePatch as a placeholder
                            content: parsed_output.draft_text,
                        };
                        let response = AgentResponse::TaskCompleted {
                            task_id: task.id.to_string(),
                            agent_id: components.id.clone(),
                            deliverable,
                        };
                        if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                            error!("Failed to send TaskCompleted response from WriterAgent: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("WriterAgent failed to parse MCP output JSON: {}", e);
                        let response = AgentResponse::TaskFailed {
                            task_id: task.id.to_string(),
                            agent_id: components.id.clone(),
                            error: format!("Failed to parse MCP output: {}", e),
                        };
                        if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                            error!("Failed to send TaskFailed response from WriterAgent: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("WriterAgent failed to invoke MCP: {}", e);
                let response = AgentResponse::TaskFailed {
                    task_id: task.id.to_string(),
                    agent_id: components.id.clone(),
                    error: format!("Failed to invoke MCP: {}", e),
                };
                if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                    error!("Failed to send TaskFailed response from WriterAgent: {}", e);
                }
            }
        }

        components.set_status(AgentStatus::Idle);
        components.current_task_id = None;
        components.current_task = None;
        Ok(())
    }
}