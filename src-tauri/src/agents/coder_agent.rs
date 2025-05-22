use crate::agents::base_agent::Agent;
use crate::agents::base_agent_components::BaseAgentComponents; // Import directly
use crate::common_types::agent_defs::{AgentCapabilities, AgentStatus, AgentRole}; // Import directly
use crate::common_types::task_graph_defs::TaskNode; // Corrected import
use crate::common_types::sprint_defs::Deliverable;
use crate::common_types::message_defs::{Message, MessageContent, SubTaskDelegationRequest, AgentResponse}; // Removed DelegateSubTask, DelegatedTaskCompletedNotification
use crate::common_types::task_defs::{TaskSpecification, InputMapping, TaskType}; // Added TaskType
use tokio::sync::{mpsc, broadcast, Mutex};
use async_trait::async_trait;
use log::{info, error};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;
use std::sync::Arc; // Added Arc import
use crate::communication_bus::BusRequest; // Import BusRequest
use crate::mcp_manager::MCPManager; // Import MCPManager

#[derive(Deserialize, Debug)]
struct CodeGenMcpOutput {
    generated_code: String,
    explanation: Option<String>,
}

/// The CoderAgent is responsible for generating or modifying code.
pub struct CoderAgent {
    components: Mutex<BaseAgentComponents>,
}

impl CoderAgent {
    /// Creates a new `CoderAgent`.
    pub async fn new(
        id: String,
        name: String,
        config: crate::AgentConfig,
        mcp_manager: Arc<MCPManager>,
        bus_request_sender: mpsc::Sender<BusRequest>,
    ) -> Result<Self, anyhow::Error> {
        let components = Mutex::new(BaseAgentComponents::new(
            id,
            name,
            AgentStatus::Idle, // Default status
            AgentCapabilities {
                can_code: true,
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
            bus_request_sender,
        ));
        Ok(Self { components })
    }
}

#[async_trait]
impl Agent for CoderAgent {
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

    async fn start(self: std::sync::Arc<Self>, mut bus_receiver: broadcast::Receiver<Message>, bus_sender: mpsc::Sender<BusRequest>) {
        info!("CoderAgent {} started.", self.id().await);
        self.set_status(AgentStatus::Idle).await;

        while let Ok(message) = bus_receiver.recv().await {
            info!("CoderAgent {} received message: {:?}", self.id().await, message);
            match message.content {
                MessageContent::TaskAssignment { task } => {
                    info!("CoderAgent {} received TaskAssignment for task {}", self.id().await, task.id);
                    // Assuming agents process tasks in process_task, this might trigger that.
                    // For now, just log. The orchestrator handles task assignment and state.
                    if let Err(e) = self.process_task(task).await {
                        error!("Error processing task for CoderAgent: {}", e);
                    }
                }
                MessageContent::DelegatedTaskCompletedNotification { completed_sub_task_id, .. } => {
                    info!("CoderAgent {} received DelegatedTaskCompletedNotification for subtask {}", self.id().await, completed_sub_task_id);
                    // If the agent was waiting for this specific subtask, resume its work.
                    // For now, just change status back to Idle.
                    self.set_status(AgentStatus::Idle).await;
                    info!("CoderAgent {} status set to Idle.", self.id().await);
                    // In a real scenario, the agent would need to check if the completed subtask
                    // is the one it was waiting for and potentially retrieve results.
                }
                // Handle other message types if necessary
                _ => {
                    info!("CoderAgent {} received unhandled message content: {:?}", self.id().await, message.content);
                }
            }
        }
        info!("CoderAgent {} shutting down.", self.id().await);
    }

    async fn process_task(&self, task: TaskNode) -> anyhow::Result<()> {
        let mut components = self.components.lock().await;
        info!("CoderAgent {} received task: {:?}", components.id, task.task_spec.task_type);

        match task.task_spec.task_type {
            TaskType::GenerateCode => {
                let instruction = task.task_spec.description.clone();
                let mut language: Option<String> = None;
                let mut context: Option<String> = None;

                if let Some(details_str) = task.task_spec.context.as_deref() { // Corrected handling of Option<String> to Option<&str>
                    match serde_json::from_str::<Value>(details_str) {
                        Ok(details_json) => {
                            if let Some(lang) = details_json.get("language").and_then(|v| v.as_str()) {
                                language = Some(lang.to_string());
                            }
                            if let Some(ctx) = details_json.get("context").and_then(|v| v.as_str()) {
                                context = Some(ctx.to_string());
                            }
                        },
                        Err(e) => {
                            error!("Failed to parse task details JSON for CoderAgent task {}: {}", task.id, e);
                            // Continue without language/context from details
                        }
                    }
                }

                let language = match language {
                    Some(lang) => lang,
                    None => {
                        error!("Language not specified for CoderAgent task {}", task.id);
                        let response = AgentResponse::TaskFailed {
                            task_id: task.id.clone().to_string(),
                            agent_id: components.id.clone(),
                            error: "Target language not specified in task details.".to_string(),
                        };
                        components.bus_sender.send(BusRequest::AgentResponse { message: response }).await?;
                        return Ok(());
                    }
                };

                let mcp_args = serde_json::json!({
                    "instruction": instruction,
                    "language": language,
                    "context": context,
                });

                match components.mcp_manager.invoke_mcp(
                    crate::common_types::mcp_defs::MCPInput {
                        mcp_id: "generate_code_v1".to_string(),
                        data: mcp_args,
                        context_overrides: None,
                    }
                ).await {
                    Ok(mcp_output) => {
                        match serde_json::from_value::<CodeGenMcpOutput>(mcp_output.processed_content.unwrap_or_default()) { // Use processed_content
                            Ok(parsed_output) => {
                                info!("Successfully generated code for task {}: {}", task.id, parsed_output.generated_code);
                                if let Some(explanation) = parsed_output.explanation {
                                    info!("Explanation: {}", explanation);
                                }

                                let deliverable = Deliverable::CodePatch {
                                    content: parsed_output.generated_code,
                                };

                                // Delegate validation sub-task
                                let validation_sub_task_spec = TaskSpecification {
                                    name: format!("Validate Code for Task {}", task.id),
                                    description: "Validate the generated code for syntax, style, and basic correctness.".to_string(),
                                    required_role: AgentRole::Validator, // Specify the required role for the subtask
                                    input_mappings: vec![
                                        InputMapping {
                                            source_task_id: task.id, // Use the current task's ID as the source
                                            deliverable_key: "generated_code".to_string(), // A key to identify this deliverable
                                            target_input_name: "code_to_validate".to_string(), // How the validator agent expects it
                                        }
                                    ],
                                    priority: None, // Optional priority
                                    required_agent_role: Some(AgentRole::Validator), // Optional more specific role requirement
                                    context: None, // Initialize context for subtasks
                                    task_type: TaskType::ValidateContent, // Added task_type
                                };

                                let delegation_request = SubTaskDelegationRequest {
                                    parent_task_id: task.id.to_string(),
                                    delegating_agent_id: components.id.clone(),
                                    sub_task_spec: validation_sub_task_spec,
                                };

                                let delegate_message = Message {
                                    id: Uuid::new_v4().to_string(),
                                    sender_id: components.id.clone(),
                                    receiver_id: None, // Sent to the orchestrator
                                    content: MessageContent::DelegateSubTask(delegation_request),
                                };

                                info!("CoderAgent {} delegating validation sub-task for task {}", components.id, task.id);
                                if let Err(e) = components.bus_sender.send(BusRequest::GeneralMessage { message: delegate_message }).await {
                                    error!("CoderAgent {} failed to send DelegateSubTask message: {}", components.id, e);
                                    // Handle error, perhaps send TaskFailed for the original task
                                    let response = AgentResponse::TaskFailed {
                                        task_id: task.id.to_string(),
                                        agent_id: components.id.clone(),
                                        error: format!("Failed to delegate validation sub-task: {}", e),
                                    };
                                    if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                                        error!("Failed to send TaskFailed response after delegation failure from CoderAgent {}: {}", components.id, e);
                                    }
                                } else {
                                    info!("CoderAgent {} successfully sent DelegateSubTask message for task {}", components.id, task.id);
                                    // The orchestrator will handle the status change to WaitingForDelegatedTask
                                    // once it processes the DelegateSubTask message.
                                    // We don't change status here directly to avoid race conditions.
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse MCP output for CoderAgent task {}: {}", task.id, e);
                                let response = AgentResponse::TaskFailed {
                                    task_id: task.id.to_string(),
                                    agent_id: components.id.clone(),
                                    error: format!("Failed to parse MCP output: {}", e),
                                };
                                if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                                    error!("Failed to send TaskFailed response from CoderAgent {}: {}", components.id, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("MCP tool invocation failed for CoderAgent task {}: {}", task.id, e);
                        let response = AgentResponse::TaskFailed {
                            task_id: task.id.to_string(),
                            agent_id: components.id.clone(),
                            error: format!("MCP tool invocation failed: {}", e),
                        };
                        if let Err(e) = components.bus_sender.send(BusRequest::AgentResponse { message: response }).await {
                            error!("Failed to send TaskFailed response from CoderAgent {}: {}", components.id, e);
                        }
                    }
                }
            }
            // Handle other TaskTypes if necessary
            _ => {
                info!("CoderAgent {} received unhandled task type: {:?}", components.id, task.task_spec.task_type);
                // Optionally send TaskFailed for unhandled task types
            }
        }
        Ok(())
    }
}