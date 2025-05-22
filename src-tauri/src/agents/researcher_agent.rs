use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::info;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc; // Import Arc

use crate::agents::base_agent::Agent;
use crate::agents::base_agent_components::BaseAgentComponents;
use crate::common_types::agent_defs::{AgentCapabilities, AgentConfig, AgentStatus};
use crate::common_types::message_defs::{Message, MessageContent, InformationResponse, AgentResponse};
use crate::common_types::sprint_defs::Deliverable;
use crate::common_types::task_graph_defs::TaskNode;
use crate::mcp_manager::MCPManager;
use crate::common_types::mcp_defs::MCPInput;
use crate::communication_bus::BusRequest;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
struct ResearchMcpOutput {
    summary: String,
    sources: Vec<String>,
}

pub struct ResearcherAgent {
    pub components: Mutex<BaseAgentComponents>,
}

impl ResearcherAgent {
    pub async fn new(
        id: String,
        name: String,
        status: AgentStatus,
        capabilities: AgentCapabilities,
        mcp_manager: std::sync::Arc<MCPManager>,
        config: AgentConfig,
        bus_request_sender: mpsc::Sender<BusRequest>,
    ) -> Result<Self, anyhow::Error> {
        let components = Mutex::new(BaseAgentComponents::new(
            id,
            name,
            status,
            capabilities,
            mcp_manager,
            config,
            bus_request_sender,
        ));
        Ok(Self { components })
    }
}

#[async_trait]
impl Agent for ResearcherAgent {
    async fn id(&self) -> String {
        let components = self.components.lock().await;
        components.id.clone()
    }

    async fn name(&self) -> String {
        let components = self.components.lock().await;
        components.name.clone()
    }

    async fn get_status(&self) -> AgentStatus {
        let components = self.components.lock().await;
        components.status.clone()
    }

    async fn set_status(&self, status: AgentStatus) {
        let mut components = self.components.lock().await;
        components.status = status;
    }

    async fn get_capabilities(&self) -> AgentCapabilities {
        let components = self.components.lock().await;
        components.capabilities.clone()
    }

    async fn get_config(&self) -> crate::common_types::AgentConfig {
        let components = self.components.lock().await;
        components.get_config()
    }

    async fn start(self: std::sync::Arc<Self>, mut bus_receiver: tokio::sync::broadcast::Receiver<crate::common_types::Message>, bus_sender: mpsc::Sender<BusRequest>) {
        info!("ResearcherAgent {} starting...", self.id().await);
        loop {
            let agent_id = self.id().await;
            let message_result = bus_receiver.recv().await;
            match message_result {
                Ok(message) => {
                    info!("ResearcherAgent {} received message in start loop: {:?}", agent_id, message);
                    match message.content {
                        MessageContent::TaskAssignment { task } => {
                             // Handle new task assignment
                            self.set_status(AgentStatus::Busy).await;
                            let task_id = task.id.clone();
                            // Process task directly, blocking the agent until completion
                            if let Err(e) = self.process_task(task).await {
                                eprintln!("Error processing task for ResearcherAgent {}: {}", agent_id, e);
                                let response = AgentResponse::TaskFailed {
                                    agent_id: agent_id.clone(),
                                    task_id: task_id.to_string(),
                                    error: format!("Task processing failed: {}", e),
                                };
                                let components = self.components.lock().await;
                                if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                                    eprintln!("Failed to send TaskFailed response from ResearcherAgent: {}", e);
                                }
                            }
                        },
                        MessageContent::RequestInformation(info_request) => {
                            info!("ResearcherAgent {} received RequestInformation: {:?}", self.id().await, info_request);
                            let components_lock = self.components.lock().await;
                            let response_sender = components_lock.bus_sender.clone();
                            let agent_id_clone = components_lock.id.clone(); // Clone for the async block
                            let original_task_id_clone = info_request.original_task_id.clone(); // Clone for the async block
                            let original_requesting_agent_id_clone = info_request.requesting_agent_id.clone(); // Clone for the async block
                            let request_query_clone = info_request.query.clone(); // Clone for the async block
                            let mcp_manager = components_lock.mcp_manager.clone(); // Clone mcp_manager for the async block
                            drop(components_lock); // Release the lock

                            tokio::spawn(async move {
                                let mcp_input = MCPInput {
                                    mcp_id: "perform_basic_research_v1".to_string(),
                                    data: json!({
                                        "query": info_request.query,
                                        "num_results_to_summarize": 3 // Using default from MCP
                                    }),
                                    context_overrides: None,
                                };

                                let mcp_result = mcp_manager.invoke_mcp(mcp_input).await;

                                let info_response_payload = match mcp_result {
                                    Ok(mcp_output) => {
                                        match serde_json::from_value::<ResearchMcpOutput>(mcp_output.processed_content.unwrap_or_default()) {
                                            Ok(parsed_output) => {
                                                InformationResponse {
                                                    original_task_id: original_task_id_clone,
                                                    original_requesting_agent_id: original_requesting_agent_id_clone.clone(),
                                                    responding_agent_id: agent_id_clone.clone(),
                                                    request_query: request_query_clone.clone(),
                                                    response_data: format!("Summary: {}\nSources: {:?}", parsed_output.summary, parsed_output.sources),
                                                    error: None,
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("ResearcherAgent failed to parse MCP output JSON: {}", e);
                                                InformationResponse {
                                                    original_task_id: original_task_id_clone,
                                                    original_requesting_agent_id: original_requesting_agent_id_clone.clone(),
                                                    responding_agent_id: agent_id_clone.clone(),
                                                    request_query: request_query_clone.clone(),
                                                    response_data: String::new(),
                                                    error: Some(format!("Failed to parse MCP output: {}", e)),
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("ResearcherAgent failed to invoke MCP: {}", e);
                                        InformationResponse {
                                            original_task_id: original_task_id_clone,
                                            original_requesting_agent_id: original_requesting_agent_id_clone.clone(),
                                            responding_agent_id: agent_id_clone.clone(),
                                            request_query: request_query_clone.clone(),
                                            response_data: String::new(),
                                            error: Some(format!("MCP invocation failed: {}", e)),
                                        }
                                    }
                                };

                                let response_message = Message {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    sender_id: agent_id_clone,
                                    receiver_id: Some(original_requesting_agent_id_clone), // Send back to the requesting agent
                                    content: MessageContent::ReturnInformation(info_response_payload),
                                };

                                if let Err(e) = response_sender.send(BusRequest::GeneralMessage { message: response_message }).await {
                                    eprintln!("Failed to send ReturnInformation response from ResearcherAgent: {}", e);
                                }
                            });
                        },
                        _ => {
                            // Ignore other message types for now
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("ResearcherAgent {} lagged behind by {} messages. Skipping some messages.", agent_id, n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!("ResearcherAgent {} channel closed. Shutting down.", agent_id);
                    break;
                }
            }
        }
    }

    async fn process_task(&self, task: TaskNode) -> anyhow::Result<()> {
        let mut components = self.components.lock().await;
        info!("ResearcherAgent received task: {:?}", task.id);

        let query = task.description.clone(); // Assuming description is the query

        let mcp_input = MCPInput {
            mcp_id: "perform_basic_research_v1".to_string(), // Added mcp_id
            data: json!({
                "query": query,
                "num_results_to_summarize": 3 // Using default from MCP
            }),
            context_overrides: None, // Added context_overrides
        };

        let mcp_result = components.mcp_manager.invoke_mcp(
            mcp_input,
        ).await;

        match mcp_result {
            Ok(mcp_output) => {
                match serde_json::from_value::<ResearchMcpOutput>(mcp_output.processed_content.unwrap_or_default()) { // Handle None case
                    Ok(parsed_output) => {
                        let deliverable = Deliverable::ResearchReport {
                            content: parsed_output.summary,
                            sources: parsed_output.sources,
                        };
                        let response = AgentResponse::TaskCompleted {
                            agent_id: components.id.clone(),
                            task_id: task.id.to_string(),
                            deliverable,
                        };
                        components.bus_sender.send(BusRequest::AgentResponse { message: response }).await?;
                    }
                    Err(e) => {
                        let response = AgentResponse::TaskFailed {
                            agent_id: components.id.clone(),
                            task_id: task.id.to_string(),
                            error: format!("Failed to parse MCP output: {}", e),
                        };
                        components.bus_sender.send(BusRequest::AgentResponse { message: response }).await?;
                        return Err(e.into());
                    }
                }
            }
            Err(e) => {
                let response = AgentResponse::TaskFailed {
                    agent_id: components.id.clone(),
                    task_id: task.id.to_string(),
                    error: format!("MCP invocation failed: {}", e),
                };
                components.bus_sender.send(BusRequest::AgentResponse { message: response }).await?;
                return Err(e.into());
            }
        }

        components.set_status(AgentStatus::Idle); // Set status back to Idle after task completion/failure
        Ok(())
    }
}