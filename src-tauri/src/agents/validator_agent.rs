use crate::agents::base_agent::Agent;
use crate::agents::base_agent_components::BaseAgentComponents;
use crate::common_types::agent_defs::{AgentConfig, AgentStatus, AgentCapabilities};
use crate::common_types::sprint_defs::{Deliverable};
use crate::common_types::task_graph_defs::TaskNode;
use crate::common_types::message_defs::{Message, AgentResponse}; // Added AgentResponse
use crate::common_types::mcp_defs::{MCPInput};
use async_trait::async_trait;
use log::{info};
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use serde_json::{json};
use tokio::sync::broadcast;
use std::sync::Arc;
use crate::communication_bus::BusRequest;

#[derive(Deserialize, Serialize, Debug)]
struct ValidationCriterionResult {
    criterion: String,
    passed: bool,
    comment: String,
}

#[derive(Deserialize, Debug)]
struct ValidationMcpOutput {
    is_valid: bool,
    feedback: String,
    criteria_results: Vec<ValidationCriterionResult>,
}

pub struct ValidatorAgent {
    components: Mutex<BaseAgentComponents>,
}

impl ValidatorAgent {
    pub async fn new(
        config: AgentConfig,
        mcp_manager: Arc<crate::mcp_manager::MCPManager>,
        bus_request_sender: mpsc::Sender<BusRequest>,
    ) -> Result<Self, anyhow::Error> {
        info!("ValidatorAgent initialized with config: {:?}", config);
        let id = config.id.clone();
        let name = config.role.to_string();
        let status = AgentStatus::Idle;
        let capabilities = AgentCapabilities {
            can_research: false,
            can_write: false,
            can_plan: false,
            can_code: false,
            can_design: false,
            can_test: false,
            can_debug: false,
            can_architect: false,
            can_manage_sprint: false,
            can_use_tool: true,
        };

        Ok(ValidatorAgent {
            components: Mutex::new(BaseAgentComponents::new(
                id,
                name,
                status,
                capabilities,
                mcp_manager,
                config,
                bus_request_sender,
            )),
        })
    }
}

#[async_trait]
impl Agent for ValidatorAgent {

    async fn get_capabilities(&self) -> AgentCapabilities {
        let components = self.components.lock().await;
        components.get_capabilities()
    }

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

    async fn get_config(&self) -> crate::common_types::AgentConfig {
        let components = self.components.lock().await;
        components.get_config()
    }

    async fn start(self: Arc<Self>, mut bus_receiver: broadcast::Receiver<Message>, bus_sender: mpsc::Sender<BusRequest>) {
        info!("ValidatorAgent {} started.", self.id().await);
        // In a real scenario, this loop would process messages from the bus.
        // For now, it's a placeholder to satisfy the Agent trait.
        loop {
            let agent_id = self.id().await;
            tokio::select! {
                _ = bus_receiver.recv() => {
                    // Process messages from the bus if needed
                },
                _ = tokio::signal::ctrl_c() => {
                    info!("ValidatorAgent {} received Ctrl-C, shutting down.", agent_id);
                    break;
                }
            }
        }
    }

    async fn process_task(
        &self,
        task: TaskNode,
    ) -> anyhow::Result<()> {
        let mut components = self.components.lock().await;
        info!(
            "ValidatorAgent {} received task: {:?} for sprint: {:?}",
            components.id, task.id, task.sprint_id
        );

        let mut content_to_validate: Option<String> = None;
        let mut validation_criteria: Vec<String> = Vec::new();
        let mut is_code_validation = false;

        // Check inputs for code content
        for input_deliverable in &task.inputs {
            // The TaskInput struct has a 'data' field of type Value.
            // We need to check if this 'data' Value contains the expected fields for validation.
            if let Some(deliverable_data) = input_deliverable.data.as_object() {
                if let Some(data_type_value) = deliverable_data.get("data_type") {
                    if let Some(data_type_str) = data_type_value.as_str() {
                        if data_type_str == "CodePatch" {
                            if let Some(value_obj) = deliverable_data.get("value") {
                                if let Some(patch_content_value) = value_obj.get("patch_content") {
                                    if let Some(patch_content) = patch_content_value.as_str() {
                                        content_to_validate = Some(patch_content.to_string());
                                        is_code_validation = true;
                                        validation_criteria = vec![
                                            "Check for syntax errors".to_string(),
                                            "Check for basic style guide violations".to_string(),
                                            "Ensure code compiles (if applicable)".to_string(),
                                        ];
                                        break;
                                    }
                                }
                            }
                        } else if data_type_str == "TextContent" {
                            if let Some(value_obj) = deliverable_data.get("value") {
                                if let Some(text_content_value) = value_obj.get("text_content") {
                                    if let Some(text_content) = text_content_value.as_str() {
                                        content_to_validate = Some(text_content.to_string());
                                        is_code_validation = false;
                                        validation_criteria = vec![
                                            "Check for clarity and coherence".to_string(),
                                            "Check for factual accuracy".to_string(),
                                        ];
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // If no code found in inputs, fall back to description and details
        if !is_code_validation {
            content_to_validate = Some(task.task_spec.description.clone());

            // TaskSpecification does not have a 'details' field.
            // If validation criteria are not derived from code, they must be explicitly provided
            // in the task description or a dedicated input deliverable.
            // For now, if not code validation, and no criteria found, proceed with empty criteria.
            info!("Task {} is not code validation and no explicit criteria found. Proceeding with empty criteria.", task.id);
        }

        let content = match content_to_validate {
            Some(content) => content,
            None => {
                let error_msg = format!("Task {} has no content to validate in description or inputs", task.id);
                log::error!("{}", error_msg);
                let response = AgentResponse::TaskFailed {
                    task_id: task.id.to_string(),
                    agent_id: components.id.clone(),
                    error: error_msg,
                };
                components.bus_sender.send(BusRequest::AgentResponse { message: response }).await?;
                return Ok(());
            }
        };

        let mut mcp_input = serde_json::json!({
            "validation_criteria": validation_criteria
        });

        if is_code_validation {
            mcp_input["code_content"] = json!(content);
        } else {
            mcp_input["text_content"] = json!(content);
        }


        let mcp_input_obj = MCPInput {
            mcp_id: "validate_content_v1".to_string(),
            data: mcp_input,
            context_overrides: None,
        };

        let mcp_result = components.mcp_manager.invoke_mcp(mcp_input_obj).await;

        match mcp_result {
            Ok(mcp_output) => {
                match mcp_output.processed_content {
                    Some(processed_content_value) => {
                        match serde_json::from_value::<ValidationMcpOutput>(processed_content_value) {
                            Ok(parsed_output) => {
                                let deliverable = Deliverable::ResearchReport { // Using ResearchReport as a placeholder
                                    content: serde_json::json!({
                                        "is_valid": parsed_output.is_valid,
                                        "feedback": parsed_output.feedback,
                                        "criteria_results": parsed_output.criteria_results,
                                    }).to_string(),
                                    sources: Vec::new(),
                                };
                                let response = AgentResponse::TaskCompleted {
                                    task_id: task.id.to_string(),
                                    agent_id: components.id.clone(),
                                    deliverable,
                                };
                                components.bus_sender.send(BusRequest::AgentResponse { message: response }).await?;
                            },
                            Err(e) => {
                                let error_msg = format!("Failed to parse MCP output for task {}: {}", task.id, e);
                                log::error!("{}", error_msg);
                                let response = AgentResponse::TaskFailed {
                                    task_id: task.id.to_string(),
                                    agent_id: components.id.clone(),
                                    error: error_msg,
                                };
                                components.bus_sender.send(BusRequest::AgentResponse { message: response }).await?;
                            }
                        }
                    },
                    None => {
                        let error_msg = format!("MCP output for task {} has no processed content.", task.id);
                        log::error!("{}", error_msg);
                        let response = AgentResponse::TaskFailed {
                            task_id: task.id.to_string(),
                            agent_id: components.id.clone(),
                            error: error_msg,
                        };
                        components.bus_sender.send(BusRequest::AgentResponse { message: response }).await?;
                    }
                }
            },
            Err(e) => {
                let error_msg = format!("MCP invocation failed for task {}: {}", task.id, e);
                log::error!("{}", error_msg);
                let response = AgentResponse::TaskFailed {
                    task_id: task.id.to_string(),
                    agent_id: components.id.clone(),
                    error: error_msg,
                };
                components.bus_sender.send(BusRequest::AgentResponse { message: response }).await?;
            }
        }

        Ok(())
    }
}