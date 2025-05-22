// src-tauri/src/core_orchestrator/mod.rs

pub mod components; // Declare the components module

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Context, Result};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid; // Import Uuid

use crate::agent_manager::AgentManager;
use crate::communication_bus::CommunicationBus;
use crate::common_types::{
    AgentConfig, TaskGraph, TaskInput,TaskSpecification
};
use crate::mcp_manager::MCPManager;
use components::message_processor::MessageProcessor;
use components::task_graph_manager::TaskGraphManager;
use components::task_result_processor::TaskResultProcessor;
use components::task_scheduler::TaskScheduler;

/// The CoreOrchestrator is responsible for the high-level coordination
/// of tasks, agents, and communication within the system.
pub struct CoreOrchestrator {
    agent_manager: Arc<Mutex<AgentManager>>,
    communication_bus: Arc<CommunicationBus>,
    mcp_manager: Arc<MCPManager>,
    task_graph_manager: Arc<Mutex<TaskGraphManager>>,
    message_processor: MessageProcessor,
    task_result_processor: TaskResultProcessor,
    task_scheduler: TaskScheduler,
}


impl CoreOrchestrator {
    /// Creates a new instance of the CoreOrchestrator.
    pub async fn new(
        communication_bus: Arc<CommunicationBus>,
        mcp_manager: Arc<MCPManager>,
        db_connection: Arc<tokio::sync::Mutex<rusqlite::Connection>>,
    ) -> Result<Self, anyhow::Error> {
        log::info!("DEBUG: [CoreOrchestrator::new] - Initializing CoreOrchestrator.");

        // Initialize shared managers
        let agent_manager = Arc::new(Mutex::new(AgentManager::new(
            Arc::clone(&mcp_manager),
            Arc::clone(&communication_bus),
        )));
        log::debug!("DEBUG: [CoreOrchestrator::new] - AgentManager initialized.");
        
        let task_graph_manager = Arc::new(Mutex::new(
            TaskGraphManager::new(db_connection).await?
        ));
        log::debug!("DEBUG: [CoreOrchestrator::new] - TaskGraphManager initialized.");

        // Create communication channels
        let (task_results_sender, task_results_receiver) = mpsc::channel(100);
        let bus_receiver = communication_bus.subscribe();
        let bus_sender = communication_bus.get_sender();
        log::debug!("DEBUG: [CoreOrchestrator::new] - Communication channels created.");

        // Initialize components with proper parameters
        let message_processor = MessageProcessor::new(
            bus_receiver,
            bus_sender.clone(),
            task_results_sender.clone(),
            Arc::clone(&agent_manager),
            Arc::clone(&task_graph_manager),
        );
        log::debug!("DEBUG: [CoreOrchestrator::new] - MessageProcessor initialized.");

        let task_result_processor = TaskResultProcessor::new(
            task_results_receiver,
            Arc::clone(&task_graph_manager),
            Arc::clone(&agent_manager),
            bus_sender.clone(),
        );
        log::debug!("DEBUG: [CoreOrchestrator::new] - TaskResultProcessor initialized.");

        let task_scheduler = TaskScheduler::new(
            Arc::clone(&task_graph_manager),
            Arc::clone(&agent_manager),
            Arc::clone(&communication_bus),
        );
        log::debug!("DEBUG: [CoreOrchestrator::new] - TaskScheduler initialized.");

        log::info!("DEBUG: [CoreOrchestrator::new] - CoreOrchestrator initialization complete.");
        Ok(Self {
            agent_manager,
            communication_bus,
            mcp_manager,
            task_graph_manager,
            message_processor,
            task_result_processor,
            task_scheduler,
        })
    }

    /// Initializes agents based on the provided configurations.
    /// Delegates to the AgentManager.
    pub async fn initialize_agents(&self, configs: Vec<AgentConfig>) -> Result<(), anyhow::Error> {
        log::info!("DEBUG: [CoreOrchestrator::initialize_agents] - Initializing agents. Configs count: {}", configs.len());
        let agent_manager = self.agent_manager.lock().await; // Removed 'mut' as it's not needed
        
        for config in configs {
            log::debug!("DEBUG: [CoreOrchestrator::initialize_agents] - Spawning agent with role: {:?}", config.role);
            let spawn_result = agent_manager.spawn_agent(
                config,
            ).await;
            
            if let Err(e) = spawn_result {
                log::error!("ERROR: [CoreOrchestrator::initialize_agents] - Failed to spawn agent: {}", e);
                continue;
            }
            log::debug!("DEBUG: [CoreOrchestrator::initialize_agents] - Agent spawned successfully.");
        }
        log::info!("DEBUG: [CoreOrchestrator::initialize_agents] - Finished initializing agents.");
        Ok(())
    }

    /// Starts the communication bus listener task.
    /// Delegates to the MessageProcessor.
    pub fn start_bus_listener(&mut self) {
        log::info!("DEBUG: [CoreOrchestrator::start_bus_listener] - Starting communication bus listener.");
        let _ = self.message_processor.start_listening(); // Added let _ = to ignore Result
        log::debug!("DEBUG: [CoreOrchestrator::start_bus_listener] - Communication bus listener started.");
    }

    /// Starts the task result processing task.
    /// Delegates to the TaskResultProcessor.
    pub async fn start_result_processing(&mut self) -> Result<()> {
        log::info!("DEBUG: [CoreOrchestrator::start_result_processing] - Starting task result processing.");
        let result = self.task_result_processor.start_processing().await;
        if let Err(ref e) = result {
            log::error!("ERROR: [CoreOrchestrator::start_result_processing] - Task result processing failed: {}", e);
        } else {
            log::debug!("DEBUG: [CoreOrchestrator::start_result_processing] - Task result processing started successfully.");
        }
        result
    }

    /// Runs the main orchestration cycle.
    /// Delegates to the TaskScheduler.
    pub async fn run_orchestration_cycle(&mut self) -> Result<()> {
        log::info!("DEBUG: [CoreOrchestrator::run_orchestration_cycle] - Running orchestration cycle.");
        let result = self.task_scheduler.run_scheduling_cycle().await;
        if let Err(ref e) = result {
            log::error!("ERROR: [CoreOrchestrator::run_orchestration_cycle] - Orchestration cycle failed: {}", e);
        } else {
            log::debug!("DEBUG: [CoreOrchestrator::run_orchestration_cycle] - Orchestration cycle completed successfully.");
        }
        result
    }

    /// Creates a new TaskGraph.
    /// Delegates to the TaskGraphManager.
    pub async fn create_task_graph(&self, name: String, description: String, overall_goal: String) -> Result<String, anyhow::Error> {
        log::info!("DEBUG: [CoreOrchestrator::create_task_graph] - Attempting to create task graph. Name: '{}', Description: '{}', Goal: '{}'", name, description, overall_goal);
        let graph_manager = self.task_graph_manager.lock().await; // Removed 'mut' as it's not needed
        let result = graph_manager.create_task_graph(name, description, overall_goal).await;
        if let Err(ref e) = result {
            log::error!("ERROR: [CoreOrchestrator::create_task_graph] - Failed to create task graph: {}", e);
        } else {
            log::info!("DEBUG: [CoreOrchestrator::create_task_graph] - Task graph created successfully with ID: {:?}", result.as_ref().unwrap());
        }
        result
    }

    /// Retrieves an immutable reference to a TaskGraph by its ID.
    /// Delegates to the TaskGraphManager.
    pub async fn get_task_graph(&self, graph_id: &str) -> Option<TaskGraph> {
        log::debug!("DEBUG: [CoreOrchestrator::get_task_graph] - Retrieving task graph with ID: {}", graph_id);
        let graph_manager = self.task_graph_manager.lock().await;
        let result = graph_manager.get_task_graph(graph_id).await.and_then(|guard| guard.get(graph_id).cloned());
        if result.is_none() {
            log::warn!("WARN: [CoreOrchestrator::get_task_graph] - Task graph with ID '{}' not found.", graph_id);
        } else {
            log::debug!("DEBUG: [CoreOrchestrator::get_task_graph] - Task graph with ID '{}' retrieved.", graph_id);
        }
        result
    }

    /// Retrieves a mutable reference to a TaskGraph by its ID.
    /// Delegates to the TaskGraphManager.
    pub async fn get_task_graph_mut(&self, graph_id: &str) -> Option<TaskGraph> {
        log::debug!("DEBUG: [CoreOrchestrator::get_task_graph_mut] - Retrieving mutable task graph with ID: {}", graph_id);
        let graph_manager = self.task_graph_manager.lock().await; // Removed 'mut' as it's not needed
        let result = graph_manager.get_task_graph_mut(graph_id).await.and_then(|mut guard| guard.get_mut(graph_id).map(|g| g.clone()));
        if result.is_none() {
            log::warn!("WARN: [CoreOrchestrator::get_task_graph_mut] - Mutable task graph with ID '{}' not found.", graph_id);
        } else {
            log::debug!("DEBUG: [CoreOrchestrator::get_task_graph_mut] - Mutable task graph with ID '{}' retrieved.", graph_id);
        }
        result
    }

    /// Adds a new TaskNode to a specified TaskGraph.
    /// Delegates to the TaskGraphManager.
    pub async fn add_task_node_to_graph(
        &self,
        graph_id: &str,
        task_spec: TaskSpecification,
        sprint_id: Option<Uuid>, // Changed to sprint_id of type Option<Uuid>
        estimated_duration_ms: Option<u64>
    ) -> Result<String, anyhow::Error> {
        log::info!("DEBUG: [CoreOrchestrator::add_task_node_to_graph] - Attempting to add task node to graph '{}'. Task name: '{}', Role: {:?}", graph_id, task_spec.name, task_spec.required_role);
        let graph_manager = self.task_graph_manager.lock().await;
        
        let result = graph_manager.add_task_node_to_graph(
            graph_id,
            task_spec,
            sprint_id, // Pass the sprint_id
            estimated_duration_ms
        ).await.context("Failed to add task node to graph");

        if let Err(ref e) = result {
            log::error!("ERROR: [CoreOrchestrator::add_task_node_to_graph] - Failed to add task node to graph '{}': {}", graph_id, e);
        } else {
            log::info!("DEBUG: [CoreOrchestrator::add_task_node_to_graph] - Task node added successfully to graph '{}' with ID: {:?}", graph_id, result.as_ref().unwrap());
        }
        result
    }

    /// Adds a new TaskEdge to a specified TaskGraph.
    /// Delegates to the TaskGraphManager.
    pub async fn add_task_edge_to_graph(
        &self,
        graph_id: &str,
        from_node_id: String,
        to_node_id: String,
        condition: Option<String>,
        data_mapping: Option<HashMap<String, String>>
    ) -> Result<String, anyhow::Error> {
        log::info!("DEBUG: [CoreOrchestrator::add_task_edge_to_graph] - Attempting to add task edge to graph '{}' from '{}' to '{}'.", graph_id, from_node_id, to_node_id);
        let graph_manager = self.task_graph_manager.lock().await; // Removed 'mut' as it's not needed
        let result = graph_manager.add_task_edge_to_graph( // Call method on the locked manager
            graph_id,
            from_node_id,
            to_node_id,
            condition,
            data_mapping,
        ).await;
        if let Err(ref e) = result {
            log::error!("ERROR: [CoreOrchestrator::add_task_edge_to_graph] - Failed to add task edge to graph '{}': {}", graph_id, e);
        } else {
            log::info!("DEBUG: [CoreOrchestrator::add_task_edge_to_graph] - Task edge added successfully to graph '{}'.", graph_id);
        }
        result
    }

    // The assign_task_node_to_agent method has been moved to TaskScheduler
    // as it's part of the scheduling logic.

    /// Runs the main orchestration loops in separate Tokio tasks.
    /// This method consumes the CoreOrchestrator instance.
    pub async fn run(self) -> Result<(), anyhow::Error> {
        log::info!("DEBUG: [CoreOrchestrator::run] - CoreOrchestrator starting main loops.");

        // Create shared components for tasks
        let mut message_processor = self.message_processor;
        let mut task_result_processor = self.task_result_processor;
        let mut task_scheduler = self.task_scheduler;

        // Spawn tasks with proper error handling
        let message_handle: tokio::task::JoinHandle<Result<(), anyhow::Error>> = tokio::spawn(async move {
            log::debug!("DEBUG: [CoreOrchestrator::run] - Spawning MessageProcessor task.");
            let _ = message_processor.start_listening(); // Added let _ = to ignore Result
            log::debug!("DEBUG: [CoreOrchestrator::run] - MessageProcessor task finished.");
            Ok::<(), anyhow::Error>(())
        });

        let result_handle: tokio::task::JoinHandle<Result<(), anyhow::Error>> = tokio::spawn(async move {
            log::debug!("DEBUG: [CoreOrchestrator::run] - Spawning TaskResultProcessor task.");
            let result = task_result_processor.start_processing().await.context("Task result processing failed");
            if let Err(ref e) = result {
                log::error!("ERROR: [CoreOrchestrator::run] - TaskResultProcessor task failed: {}", e);
            } else {
                log::debug!("DEBUG: [CoreOrchestrator::run] - TaskResultProcessor task finished.");
            }
            result
        });

        let scheduler_handle: tokio::task::JoinHandle<Result<(), anyhow::Error>> = tokio::spawn(async move {
            log::debug!("DEBUG: [CoreOrchestrator::run] - Spawning TaskScheduler task.");
            loop {
                log::debug!("DEBUG: [CoreOrchestrator::run] - TaskScheduler: Running scheduling cycle.");
                if let Err(e) = task_scheduler.run_scheduling_cycle().await {
                    log::error!("ERROR: [CoreOrchestrator::run] - Task scheduling cycle failed: {}", e);
                    return Err(e);
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
            #[allow(unreachable_code)]
            Ok(())
        });

        // Wait for all tasks to complete
        log::info!("DEBUG: [CoreOrchestrator::run] - Waiting for orchestration tasks to complete.");
        tokio::select! {
            res = message_handle => {
                log::debug!("DEBUG: [CoreOrchestrator::run] - Message processor task completed.");
                let _ = res.context("Message processor task failed")?;
            },
            res = result_handle => {
                log::debug!("DEBUG: [CoreOrchestrator::run] - Task result processor task completed.");
                let _ = res.context("Task result processor task failed")?;
            },
            res = scheduler_handle => {
                log::debug!("DEBUG: [CoreOrchestrator::run] - Task scheduler task completed.");
                let _ = res.context("Task scheduler task failed")?;
            },
        }
        log::info!("DEBUG: [CoreOrchestrator::run] - CoreOrchestrator main loops finished.");
        Ok(())
    }
}
