use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc, Mutex};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error; // Added for anyhow::Error conversion

use crate::common_types::{Message, TaskNode, AgentConfig, Deliverable, AgentStatus, AgentCapabilities, DeliverableStatus, generate_id, MCPInput, TaskSpecification, SubTaskDefinition, SubTaskEdgeDefinition, AgentRole, InputMapping}; // Added InputMapping
use crate::common_types::task_defs::TaskType; // Explicitly import TaskType
use crate::common_types::task_graph_defs::TaskEdgeType; // Corrected import path for TaskEdgeType
use crate::common_types::message_defs::{MessageContent, AgentResponse}; // Import MessageContent enum and AgentResponse
use crate::mcp_manager::MCPManager;
use super::base_agent::Agent;
use crate::agents::base_agent_components::BaseAgentComponents;
use crate::communication_bus::BusRequest; // Import BusRequest

pub struct PlannerAgent {
    components: Mutex<BaseAgentComponents>,
}

impl PlannerAgent {
    pub async fn new(
        id: String,
        name: String,
        config: AgentConfig,
        mcp_manager: Arc<MCPManager>,
        bus_request_sender: mpsc::Sender<crate::communication_bus::BusRequest>, // Added bus_request_sender
    ) -> Result<Self, anyhow::Error> {
        let components = Mutex::new(BaseAgentComponents::new(
            id,
            name,
            AgentStatus::Idle, // Initial status
            AgentCapabilities { // Updated capabilities initialization
                can_research: false,
                can_write: false,
                can_plan: true,
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
            bus_request_sender, // Pass bus_request_sender to BaseAgentComponents::new
        ));
        Ok(PlannerAgent { components })
    }
}

#[async_trait]
impl Agent for PlannerAgent {
    async fn get_config(&self) -> crate::AgentConfig {
        let components = self.components.lock().await;
        components.config.clone()
    }
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

    async fn start(self: Arc<Self>, mut bus_receiver: broadcast::Receiver<Message>, bus_sender: mpsc::Sender<BusRequest>) {
        println!("PlannerAgent {} starting...", self.id().await);
        // Basic message processing loop - will be expanded later
        loop {
            let agent_id = self.id().await;
            match bus_receiver.recv().await {
                Ok(message) => {
                    println!("PlannerAgent {} received message: {:?}", agent_id, message);
                    // Add message handling logic here later
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("PlannerAgent {} lagged behind by {} messages. Skipping some messages.", agent_id, n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    println!("PlannerAgent {} channel closed. Shutting down.", agent_id);
                    break;
                }
            }
        }
    }

    async fn process_task(&self, task: TaskNode) -> Result<(), anyhow::Error> {
        let mut components = self.components.lock().await;
        println!("PlannerAgent {} received task to process: {:?}", components.id, task);

        let objective = task.description.clone();
        let context = task.task_spec.context.clone(); // Use task.task_spec.context

        let mcp_input = MCPInput {
            mcp_id: "decompose_task_v1".to_string(),
            data: serde_json::json!({
                "objective": objective,
                "context": context,
            }),
            context_overrides: None,
        };

        let mcp_result = components.mcp_manager.invoke_mcp(
            mcp_input,
        ).await;

        match mcp_result {
            Ok(mcp_output) => {
                #[derive(Deserialize)]
                struct RawSubTask {
                    title: String,
                    description: String,
                    dependencies: Vec<String>,
                }
                #[derive(Deserialize)]
                struct McpOutputFormat {
                    subtasks: Vec<RawSubTask>,
                }

                match serde_json::from_value::<McpOutputFormat>(mcp_output.processed_content.unwrap_or_default()) { // Use processed_content
                    Ok(parsed_output) => {
                        let mut sub_tasks: Vec<SubTaskDefinition> = Vec::new();
                        let mut sub_task_edges: Vec<SubTaskEdgeDefinition> = Vec::new();
                        let mut temp_id_map: HashMap<String, String> = HashMap::new();

                        for raw_subtask in parsed_output.subtasks {
                            let temp_id = raw_subtask.title.clone(); // Using title as temp_id, assuming uniqueness
                            temp_id_map.insert(raw_subtask.title.clone(), temp_id.clone());

                            let task_spec = TaskSpecification {
                                name: raw_subtask.title.clone(),
                                description: raw_subtask.description.clone(),
                                required_role: AgentRole::SimpleWorker, // Default role, can be refined later
                                input_mappings: Vec::new(), // Use input_mappings instead of inputs
                                priority: None, // Priority can be set later
                                required_agent_role: None, // Add the new field
                                context: None, // Initialize context for subtasks
                                task_type: TaskType::Generic, // Added task_type
                            };

                            sub_tasks.push(SubTaskDefinition {
                                title: raw_subtask.title,
                                description: raw_subtask.description,
                                task_spec,
                                temp_id: temp_id.clone(),
                            });

                            for dependency_title in raw_subtask.dependencies {
                                // Assuming dependency_title exists as a title of another subtask
                                // In a real scenario, you might want to handle cases where a dependency is not found
                                sub_task_edges.push(SubTaskEdgeDefinition {
                                    from_subtask_temp_id: dependency_title, // This should be the temp_id of the dependency
                                    to_subtask_temp_id: temp_id.clone(),
                                    edge_type: TaskEdgeType::Dependency, // Assuming a simple dependency type
                                });
                            }
                        }

                        // Adjust edge 'from' IDs to use temp_ids from the map
                        for edge in &mut sub_task_edges {
                            if let Some(from_temp_id) = temp_id_map.get(&edge.from_subtask_temp_id) {
                                edge.from_subtask_temp_id = from_temp_id.clone();
                            } else {
                                eprintln!("PlannerAgent {} Warning: Dependency title '{}' not found among generated subtasks.", components.id, edge.from_subtask_temp_id);
                                // Decide how to handle missing dependencies - for now, leave as is or remove edge
                            }
                        }


                        let response_message = Message {
                            id: generate_id(),
                            sender_id: self.id().await,
                            receiver_id: None, // Sent to the orchestrator
                            content: MessageContent::SubTasksGenerated {
                                original_task_id: task.id.to_string(),
                                sub_tasks,
                                sub_task_edges,
                            },
                        };

                        if let Err(e) = components.bus_sender.send(BusRequest::GeneralMessage { message: response_message }).await {
                            eprintln!("PlannerAgent {} failed to send SubTasksGenerated response: {}", components.id, e);
                        } else {
                            println!("PlannerAgent {} sent SubTasksGenerated for task {}", components.id, task.id);
                        }
                    }
                    Err(e) => {
                        eprintln!("PlannerAgent {} failed to parse MCP output for task {}: {}", components.id, task.id, e);
                        let response = AgentResponse::TaskFailed {
                            task_id: task.id.to_string(),
                            agent_id: self.id().await,
                            error: format!("Failed to parse MCP output: {}", e),
                        };
                        if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                            eprintln!("PlannerAgent {} failed to send TaskFailed response after parsing error: {}", components.id, e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("PlannerAgent {} failed to invoke MCP for task {}: {}", components.id, task.id, e);
                let response = AgentResponse::TaskFailed {
                    task_id: task.id.to_string(),
                    agent_id: self.id().await,
                    error: format!("Failed to invoke MCP: {}", e),
                };
                if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                    eprintln!("PlannerAgent {} failed to send TaskFailed response after MCP invocation error: {}", components.id, e);
                }
            }
        }
        Ok(())
    }
}
