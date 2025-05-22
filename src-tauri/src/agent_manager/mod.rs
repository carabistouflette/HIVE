use tokio::sync::{broadcast, mpsc}; // Import mpsc
use crate::agents::{Agent, SimpleWorkerAgent, PlannerAgent}; // Import PlannerAgent
use crate::common_types::{AgentConfig, Message, AgentRole, AgentStatus}; // Import AgentRole and AgentStatus
use std::sync::Arc;
use tokio::sync::RwLock; // Import RwLock
use std::collections::HashMap; // Import HashMap
use crate::mcp_manager::MCPManager; // Import MCPManager
use crate::communication_bus::{CommunicationBus, BusRequest}; // Import CommunicationBus and BusRequest
use crate::common_types::agent_defs::AgentCapabilities; // Import AgentCapabilities

pub struct AgentManager {
    mcp_manager: Arc<MCPManager>, // Store MCPManager
    communication_bus: Arc<CommunicationBus>, // Store CommunicationBus
    // Temporary storage for agents until StateManager is implemented
    agents: RwLock<HashMap<String, Arc<dyn Agent + Send + Sync>>>,
}

impl AgentManager {
    /// Creates a new instance of the AgentManager.
    /// Accepts Arc<MCPManager> and Arc<CommunicationBus>.
    pub fn new(mcp_manager: Arc<MCPManager>, communication_bus: Arc<CommunicationBus>) -> Self {
        AgentManager {
            mcp_manager,
            communication_bus,
            agents: RwLock::new(HashMap::new()), // Initialize the agents map
        }
    }

    /// Spawns a new agent.
    /// The agent's start method is run in a separate tokio task.
    pub async fn spawn_agent(
        &self,
        agent_config: AgentConfig,
    ) -> Result<(), anyhow::Error> { // Changed return type to Result
        let agent_id = agent_config.id.clone();
        println!("Spawning agent with ID: {}", agent_id);

        // Create a new receiver for this agent from the communication bus
        let bus_receiver = self.communication_bus.subscribe();
        // Get the sender for BusRequests from the communication bus for agents to use
        let bus_sender_for_agents = self.communication_bus.get_bus_request_sender();

        // Create the agent instance as a Box<dyn Agent> based on role
        let agent_box: Box<dyn Agent + Send + Sync> = match agent_config.role {
            AgentRole::SimpleWorker => {
                println!("Creating SimpleWorkerAgent with ID: {}", agent_id);
                Box::new(SimpleWorkerAgent::new(
                    agent_id.clone(),
                    agent_config,
                    bus_sender_for_agents.clone(),
                    Arc::clone(&self.mcp_manager),
                ).await?) // Propagate error
            },
            AgentRole::Planner => {
                println!("Creating PlannerAgent with ID: {}", agent_id);
                let agent_name = format!("Planner-{}", agent_id);
                Box::new(PlannerAgent::new(
                    agent_id.clone(),
                    agent_name,
                    agent_config,
                    Arc::clone(&self.mcp_manager),
                    bus_sender_for_agents.clone(),
                ).await?) // Propagate error
            },
            AgentRole::Researcher => {
                println!("Creating ResearcherAgent with ID: {}", agent_id);
                let agent_name = format!("Researcher-{}", agent_id);
                Box::new(crate::agents::ResearcherAgent::new(
                    agent_id.clone(),
                    agent_name,
                    AgentStatus::Idle,
                    AgentCapabilities {
                        can_research: true,
                        can_write: false,
                        can_plan: false,
                        can_code: false,
                        can_design: false,
                        can_test: false,
                        can_debug: false,
                        can_architect: false,
                        can_manage_sprint: false,
                        can_use_tool: false,
                    },
                    Arc::clone(&self.mcp_manager),
                    agent_config.clone(),
                    bus_sender_for_agents.clone(),
                ).await?) // Propagate error
            },
            AgentRole::Writer => {
                println!("Creating WriterAgent with ID: {}", agent_id);
                Box::new(crate::agents::writer_agent::WriterAgent::new(
                    agent_id.clone(),
                    format!("Writer-{}", agent_id),
                    agent_config.clone(),
                    Arc::clone(&self.mcp_manager),
                    bus_sender_for_agents.clone(),
                ).await?) // Propagate error
            },
            AgentRole::Coder => {
                println!("Creating CoderAgent with ID: {}", agent_id);
                let agent_name = format!("Coder-{}", agent_id);
                Box::new(crate::agents::coder_agent::CoderAgent::new(
                    agent_id.clone(),
                    agent_name,
                    agent_config.clone(),
                    Arc::clone(&self.mcp_manager),
                    bus_sender_for_agents.clone(),
                ).await?) // Propagate error
            },
            AgentRole::Validator => {
                println!("Creating ValidatorAgent with ID: {}", agent_id);
                Box::new(crate::agents::validator_agent::ValidatorAgent::new(
                    agent_config.clone(),
                    Arc::clone(&self.mcp_manager),
                    bus_sender_for_agents.clone(),
                ).await?) // Propagate error
            },
            _ => {
                eprintln!("Unsupported agent role: {:?}", agent_config.role);
                Box::new(SimpleWorkerAgent::new(
                    agent_id.clone(),
                    agent_config,
                    bus_sender_for_agents.clone(),
                    Arc::clone(&self.mcp_manager),
                ).await?) // Propagate error
            }
        };

        // Wrap the agent in Arc for shared ownership
        let agent_arc: Arc<dyn Agent + Send + Sync> = Arc::from(agent_box);

        // Insert the agent into the temporary agents map
        self.agents.write().await.insert(agent_id.clone(), Arc::clone(&agent_arc));

        // Spawn a task for the agent to start listening on the bus
        let agent_id_clone = agent_id.clone();
        let bus_sender_clone = bus_sender_for_agents.clone(); // Corrected to use bus_sender_for_agents
        // Move the Arc<dyn Agent> into the spawned task
        tokio::spawn(async move {
            println!("Agent {} task started.", agent_id_clone);
            // Call the start method on the Arc
            agent_arc.start(bus_receiver, bus_sender_clone).await; // The start method is async
            println!("Agent {} task finished.", agent_id_clone);
        });

        println!("Agent {} spawned and task initiated.", agent_id);
        Ok(()) // Return Ok on success
    }

    /// Placeholder method to get the status of an agent.
    /// This method is no longer functional as agents are not stored.
    pub fn get_agent_status(&self, _agent_id: &str) -> Option<String> {
        None // Return None as agents are not stored
    }

    // Optional: Add a method to get an agent by ID if needed later
    // This method is no longer functional as agents are not stored.
    pub fn get_agent(&self, _agent_id: &str) -> Option<&Box<dyn Agent + Send + Sync>> {
        None // Return None as agents are not stored
    }
    /// Finds an available agent, optionally filtering by role.
    /// Note: In this architecture, agents run as independent tasks.
    /// The actual logic to find an available agent based on status and capabilities
    /// would typically involve querying a shared state or communicating via the bus.
    /// This method serves as the interface for the orchestrator.
    pub async fn find_available_agent(&self, task_id: &str, required_role: Option<AgentRole>) -> Option<Arc<dyn Agent + Send + Sync>> {
        println!("Attempting to find available agent for task {} with required role: {:?}", task_id, required_role);

        // Iterate through the temporarily stored agents
        let agents_read_guard = self.agents.read().await;
        for agent in agents_read_guard.values() {
            // Check if the agent is Idle or Ready by calling get_status()
            let status = agent.get_status().await; // get_status is now async
            if status == AgentStatus::Idle || status == AgentStatus::Ready {
                // Check if the agent's role matches the required role, if specified
                if let Some(role_needed) = &required_role {
                    if agent.get_config().await.role == *role_needed {
                        println!("Found suitable agent {} with role {:?} for task {}", agent.id().await, agent.get_config().await.role, task_id);
                        return Some(Arc::clone(agent)); // Return a clone of the Arc
                    } else {
                        println!("Agent {} role {:?} does not match required role {:?} for task {}", agent.id().await, agent.get_config().await.role, required_role, task_id);
                    }
                } else {
                    // No required role specified, any idle/ready agent is suitable
                    println!("Found suitable agent {} with role {:?} for task {} (no specific role required)", agent.id().await, agent.get_config().await.role, task_id);
                    return Some(Arc::clone(agent)); // Return a clone of the Arc
                }
            } else {
                println!("Agent {} is not available (status: {:?}) for task {}", agent.id().await, status, task_id);
            }
        }

        println!("No suitable agent found for task {} with required role: {:?}", task_id, required_role);
        None // No suitable agent found
    }
}