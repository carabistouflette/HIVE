use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc, Mutex};
use std::sync::Arc;
use crate::common_types::{Message, MessageContent, generate_id, AgentConfig};
use crate::common_types::agent_defs::{AgentStatus, AgentCapabilities};
use crate::common_types::task_graph_defs::{TaskNode};
use crate::communication_bus::BusRequest;
use super::base_agent::Agent;
use serde_json::Value;

use crate::mcp_manager::MCPManager;

use crate::agents::base_agent_components::BaseAgentComponents;

pub struct SimpleWorkerAgent {
    components: Mutex<BaseAgentComponents>,
}

impl SimpleWorkerAgent {
    pub async fn new(id: String, config: AgentConfig, bus_sender: mpsc::Sender<BusRequest>, mcp_manager: Arc<MCPManager>) -> Result<Self, anyhow::Error> {
        let components = Mutex::new(BaseAgentComponents::new(
            id.clone(),
            "SimpleWorkerAgent".to_string(), // Default name
            AgentStatus::Idle, // Default status
            AgentCapabilities {
                can_code: false,
                can_research: false,
                can_plan: false,
                can_write: false,
                can_design: false,
                can_test: false,
                can_debug: false,
                can_architect: false,
                can_manage_sprint: false,
                can_use_tool: false,
            },
            mcp_manager,
            config,
            bus_sender,
        ));
        Ok(SimpleWorkerAgent { components })
    }
}

#[async_trait]
impl Agent for SimpleWorkerAgent {
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

    async fn get_config(&self) -> crate::common_types::AgentConfig {
        let components = self.components.lock().await;
        components.get_config()
    }

    async fn process_task(&self, task: TaskNode) -> Result<(), anyhow::Error> {
        let mut components = self.components.lock().await;
        println!("Agent {} processing task: {:?}", components.id, task);

        // Update agent status to Busy
        components.set_status(AgentStatus::Busy);

        // Simulate starting work and send status update
        let status_message = Message {
            id: generate_id(),
            sender_id: components.id.clone(),
            receiver_id: task.assigned_agent_id.clone(), // Reply to the assigned agent
            content: MessageContent::StatusUpdate {
                agent_id: components.id.clone(),
                status: "Busy".to_string(), // Use "Busy" as per AgentStatus enum
                message: Some(format!("Starting task: {}", task.task_spec.description)),
            },
        };
        if let Err(e) = components.bus_sender.send(BusRequest::GeneralMessage { message: status_message }).await {
            eprintln!("Agent {} failed to send StatusUpdate: {}", components.id, e);
        } else {
            println!("Agent {} sent StatusUpdate: Busy for task {}", components.id, task.id);
        }

        // Use MCPManager to invoke an MCP
        println!("Agent {} attempting to invoke MCP.", components.id);

        let mcp_input_data = serde_json::to_value(&task.inputs).unwrap_or_else(|_| Value::Null);

        let mcp_input = crate::common_types::mcp_defs::MCPInput {
            mcp_id: task.mcp_id.clone().unwrap_or_default(), // Use mcp_id from TaskNode, provide default if None
            data: mcp_input_data,
            context_overrides: None, // Assuming context_overrides is not part of TaskNode yet
        };

        let mcp_manager = Arc::clone(&components.mcp_manager);
        let agent_id_for_response = components.id.clone();
        let task_id_for_response = task.id.to_string();
        let _assigned_agent_id_for_response = task.assigned_agent_id.clone(); // Unused variable

        match mcp_manager.invoke_mcp(mcp_input).await {
            Ok(output) => {
                println!("Agent received MCP output: {:?}", output.processed_content);
                let response = crate::common_types::message_defs::AgentResponse::TaskCompleted {
                    task_id: task_id_for_response.clone(),
                    agent_id: agent_id_for_response.clone(),
                    deliverable: crate::common_types::sprint_defs::Deliverable::ResearchReport { // Using ResearchReport as a placeholder
                        content: output.processed_content.unwrap_or_default().to_string(),
                        sources: Vec::new(),
                    },
                };
                if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                    eprintln!("Agent {} failed to send TaskCompleted response: {}", agent_id_for_response, e);
                } else {
                    println!("Agent {} sent TaskCompleted for task {}", agent_id_for_response, task_id_for_response);
                }
            }
            Err(e) => {
                eprintln!("Agent failed to invoke MCP: {}", e);
                let response = crate::common_types::message_defs::AgentResponse::TaskFailed {
                    task_id: task_id_for_response.clone(),
                    agent_id: agent_id_for_response.clone(),
                    error: format!("Failed to invoke MCP: {}", e),
                };
                if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                    eprintln!("Agent {} failed to send TaskFailed response: {}", agent_id_for_response, e);
                } else {
                    println!("Agent {} sent TaskFailed for task {}", agent_id_for_response, task_id_for_response);
                }
                return Err(e); // anyhow::Error already matches
            }
        }

        components.set_status(AgentStatus::Idle); // Set status back to Idle after task completion/failure
        Ok(())
    }

    async fn start(self: Arc<Self>, mut bus_receiver: broadcast::Receiver<Message>, bus_sender: mpsc::Sender<BusRequest>) {
        println!("Agent {} starting...", self.id().await);
        loop {
            let agent_id = self.id().await;
            match bus_receiver.recv().await {
                Ok(message) => {
                    println!("Agent {} received message in start loop: {:?}", agent_id, message);

                    // Process the message content directly here
                    if message.receiver_id.is_none() || message.receiver_id == Some(agent_id.clone()) {
                        match message.content {
                            MessageContent::TaskAssignment { task } => {
                                println!("Agent {} received task: {:?}", agent_id, task);

                                // Acknowledge task reception
                                let ack_message = Message {
                                    id: generate_id(),
                                    sender_id: agent_id.clone(),
                                    receiver_id: Some(message.sender_id.clone()), // Reply to the sender (Orchestrator)
                                    content: MessageContent::TaskAcknowledgement {
                                        task_id: task.id.to_string(), // Convert Uuid to String
                                        agent_id: agent_id.clone(),
                                    },
                                };
                                // Use the bus_sender from components
                                let components = self.components.lock().await;
                                if let Err(e) = components.bus_sender.send(BusRequest::GeneralMessage { message: ack_message }).await {
                                    eprintln!("Agent {} failed to send TaskAcknowledgement: {}", agent_id, e);
                                } else {
                                    println!("Agent {} sent TaskAcknowledgement for task {}", agent_id, task.id);
                                }

                                // Call process_task directly
                                if let Err(e) = self.process_task(task).await {
                                    eprintln!("Agent {} failed to process task: {}", agent_id, e);
                                }
                            }
                            _ => {
                                println!("Agent {} ignoring message with content: {:?}", agent_id, message.content);
                            }
                        }
                    } else {
                        println!("Agent {} ignoring message not addressed to it: {:?}", agent_id, message);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("Agent {} lagged behind by {} messages. Skipping some messages.", agent_id, n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    println!("Agent {} channel closed. Shutting down.", agent_id);
                    break;
                }
            }
        }
    }
}