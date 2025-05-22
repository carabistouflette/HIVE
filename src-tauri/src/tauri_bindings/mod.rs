use tauri::State;
use tokio::sync::Mutex; // Changed to tokio::sync::Mutex
use std::sync::Arc;
use uuid::Uuid; // Import Uuid
use crate::core_orchestrator::CoreOrchestrator;
use crate::common_types::{AgentConfig, AgentRole, TaskSpecification};
use crate::common_types::generate_id;
use anyhow::anyhow; // Import anyhow for error handling

// Placeholder for Tauri commands
#[tauri::command]
pub async fn execute_agent_task(
    state: State<'_, crate::AppState>, // Change to AppState
    prompt: String,
    agent: String,
    llm_model: String,
) -> Result<String, String> {
    log::info!("DEBUG: [tauri_bindings::execute_agent_task] - Tauri command received. Prompt: '{}', Agent: '{}', LLM Model: '{}'", prompt, agent, llm_model);
    let orchestrator_arc = state.orchestrator.clone(); // Access orchestrator from AppState

    log::debug!("DEBUG: [tauri_bindings::execute_agent_task] - Attempting to acquire orchestrator lock.");
    let result = {
        let mut orchestrator = orchestrator_arc.lock().await;
        log::debug!("DEBUG: [tauri_bindings::execute_agent_task] - Orchestrator lock acquired.");

        // 1. Initialize agent based on selection
        let agent_role = match agent.as_str() {
            "WriterAgent" => AgentRole::Writer,
            "SimpleWorker" => AgentRole::SimpleWorker,
            "CoderAgent" => AgentRole::Coder,
            "PlannerAgent" => AgentRole::Planner,
            "ResearcherAgent" => AgentRole::Researcher,
            "ValidatorAgent" => AgentRole::Validator,
            _ => {
                log::error!("ERROR: [tauri_bindings::execute_agent_task] - Invalid agent role selected: {}", agent);
                return Err(anyhow!("Invalid agent role selected: {}", agent).to_string());
            }
        };
        log::debug!("DEBUG: [tauri_bindings::execute_agent_task] - Agent role determined: {:?}", agent_role);

        let agent_configs = vec![
            AgentConfig {
                id: generate_id(),
                role: agent_role.clone(),
                llm_model: Some(llm_model.clone()),
                llm_provider_name: Some("OpenRouter".to_string()),
                specialized_config: None,
            }
        ];
        log::debug!("DEBUG: [tauri_bindings::execute_agent_task] - Initializing agents with config: {:?}", agent_configs);
        orchestrator.initialize_agents(agent_configs.clone()).await.map_err(|e| {
            log::error!("ERROR: [tauri_bindings::execute_agent_task] - Failed to initialize agents: {}", e);
            e.to_string()
        })?;
        log::info!("DEBUG: [tauri_bindings::execute_agent_task] - Agent initialized successfully: {:?}", agent_role);

        // 2. Create a new task graph
        log::debug!("DEBUG: [tauri_bindings::execute_agent_task] - Attempting to create task graph.");
        let graph_id = orchestrator.create_task_graph(
            format!("Task Graph for {}", prompt),
            format!("Graph for task: {}", prompt),
            prompt.clone(),
        ).await.map_err(|e| {
            log::error!("ERROR: [tauri_bindings::execute_agent_task] - Failed to create task graph: {}", e);
            e.to_string()
        })?;
        log::info!("DEBUG: [tauri_bindings::execute_agent_task] - Created task graph with ID: {}", graph_id);

        // 3. Add a task node to the graph
        let task_spec = TaskSpecification {
            name: format!("Execute {} with prompt: {}", agent, prompt),
            description: prompt.clone(),
            required_role: agent_role.clone(),
            input_mappings: vec![],
            priority: Some(1),
            required_agent_role: Some(agent_role.clone()),
            context: Some(prompt.clone()), // Pass the prompt as context
            task_type: crate::common_types::task_defs::TaskType::Generic,
        };
        log::debug!("DEBUG: [tauri_bindings::execute_agent_task] - Attempting to add task node to graph. Task spec: {:?}", task_spec);

        let task_id = orchestrator.add_task_node_to_graph(
            &graph_id,
            task_spec, // Pass the entire TaskSpecification
            None::<Uuid>, // No sprint ID, explicitly type as Option<Uuid>
            None, // No estimated duration
        ).await.map_err(|e| {
            log::error!("ERROR: [tauri_bindings::execute_agent_task] - Failed to add task node to graph: {}", e);
            e.to_string()
        })?;
        log::info!("DEBUG: [tauri_bindings::execute_agent_task] - Added task node with ID: {} to graph {}.", task_id, graph_id);

        // For now, we'll just return a success message. The actual task execution
        // will happen in the background via the orchestration cycle.
        let success_message = format!("Task submitted successfully. Task ID: {}", task_id);
        log::info!("DEBUG: [tauri_bindings::execute_agent_task] - Returning success: {}", success_message);
        Ok(success_message)
    };
    log::debug!("DEBUG: [tauri_bindings::execute_agent_task] - Releasing orchestrator lock and returning result.");
    result
}