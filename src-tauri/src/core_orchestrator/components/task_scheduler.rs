// src-tauri/src/core_orchestrator/components/task_scheduler.rs

use crate::agent_manager::AgentManager;
use crate::communication_bus::CommunicationBus;
use crate::common_types::{TaskNode, TaskStatus, TaskInput, Deliverable}; // Import Deliverable
use uuid::Uuid; // Import Uuid
use serde_json::Value; // Import Value
use crate::core_orchestrator::components::task_graph_manager::TaskGraphManager;
use anyhow::Result;
use std::sync::Arc; // Use Mutex for interior mutability with Arc

/// Responsible for identifying tasks ready for execution and assigning them to agents.
pub struct TaskScheduler {
    // Use Arc<Mutex<>> for shared mutable access to TaskGraphManager
    task_graph_manager: Arc<tokio::sync::Mutex<TaskGraphManager>>,
    agent_manager: Arc<tokio::sync::Mutex<AgentManager>>, // AgentManager also needs shared mutable access
    communication_bus: Arc<CommunicationBus>, // CommunicationBus is already Arc
}

impl TaskScheduler {
    /// Creates a new TaskScheduler.
    pub fn new(
        task_graph_manager: Arc<tokio::sync::Mutex<TaskGraphManager>>,
        agent_manager: Arc<tokio::sync::Mutex<AgentManager>>,
        communication_bus: Arc<CommunicationBus>,
    ) -> Self {
        TaskScheduler {
            task_graph_manager,
            agent_manager,
            communication_bus,
        }
    }

    /// Runs the main scheduling cycle.
    /// This function would typically run in a loop, checking for tasks
    /// ready for execution and assigning them to available agents.
    pub async fn run_scheduling_cycle(&mut self) -> Result<()> {
        log::info!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Task scheduling cycle started.");

        // This is a simplified placeholder. A real cycle would:
        // 1. Periodically check active_task_graphs for tasks with status ReadyToExecute.
        // 2. For each ready task, call agent_manager.find_available_agent.
        // 3. If an agent is found, update the task status to InProgress and assign it.
        // 4. Handle cases where no agent is available.

        // Example of how find_available_agent would be called:
        // Assuming 'task' is a TaskNode with status ReadyToExecute

        // Lock the managers for access
        // Collect tasks to schedule first.
        log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Attempting to collect tasks to schedule.");
        let tasks_to_schedule: Vec<(String, TaskNode)> = {
            let graph_manager_guard = self.task_graph_manager.lock().await; // Lock is held for the outer block
            log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Acquired TaskGraphManager lock for reading active graphs.");
            let mut active_graphs_map_guard = graph_manager_guard.get_active_graphs_mut().await; // This is likely an RwLockWriteGuard
            log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Acquired active graphs map guard.");

            // Use active_graphs_map_guard while graph_manager_guard is still alive
            let collected_tasks: Vec<(String, TaskNode)> = (&mut *active_graphs_map_guard)
                .iter_mut()
                .filter_map(|(graph_id, graph)| {
                    let ready_tasks: Vec<(String, TaskNode)> = graph
                        .nodes
                        .values()
                        .filter(|node| node.status == TaskStatus::ReadyToExecute)
                        .map(|node| (graph_id.clone(), node.clone())) // Clone TaskNode to avoid moving out of the guard
                        .collect();
                    if ready_tasks.is_empty() {
                        None::<Vec<(String, TaskNode)>>
                    } else {
                        Some(ready_tasks)
                    }
                })
                .flatten()
                .collect();
            log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Collected {} tasks ready for scheduling.", collected_tasks.len());
            collected_tasks
        }; // tokio::sync::RwLockWriteGuard (active_graphs_map_guard) is dropped here


        for (graph_id_str, task_node_clone) in tasks_to_schedule.into_iter() { // Use into_iter() to consume owned values
            log::info!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Attempting to schedule task '{}' from graph '{}'.", task_node_clone.id, graph_id_str);

            let mut task_inputs: Vec<TaskInput> = Vec::new();
            let mut input_mapping_failed = false;

            // Process input mappings
            log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Processing input mappings for task '{}'.", task_node_clone.id);
            for mapping in &task_node_clone.task_spec.input_mappings {
                log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Processing mapping: source_task_id='{}', deliverable_key='{}'", mapping.source_task_id, mapping.deliverable_key);
                let source_task_node_opt: Option<TaskNode> = {
                    let graph_manager_guard = self.task_graph_manager.lock().await; // Lock is held for this block
                    log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Acquired TaskGraphManager lock for reading source task node.");

                    // Assuming get_task_graph returns a guard to a map, or similar structure needing a .get()
                    let active_graphs_map_read_guard_opt = graph_manager_guard.get_task_graph(&graph_id_str).await;
                    
                    if let Some(active_graphs_map_read_guard) = active_graphs_map_read_guard_opt { // active_graphs_map_read_guard is likely an RwLockReadGuard
                        active_graphs_map_read_guard
                            .get(&graph_id_str)
                            .and_then(|graph| graph.nodes.get(&mapping.source_task_id.to_string()).cloned())
                    } else {
                        None
                    }
                };

                match source_task_node_opt {
                    Some(source_node) => {
                        log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Source task node '{}' found.", source_node.id);
                        let found_deliverable = source_node.outputs.iter().find(|d| {
                            match d {
                                crate::common_types::Deliverable::ResearchReport { content, sources: _ } => content == &mapping.deliverable_key,
                                crate::common_types::Deliverable::CodePatch { content } => content == &mapping.deliverable_key,
                            }
                        });
                        match found_deliverable {
                            Some(deliverable) => {
                                let (content, deliverable_type_name) = match deliverable {
                                    crate::common_types::Deliverable::ResearchReport { content, sources: _ } => (content.clone(), "ResearchReport"),
                                    crate::common_types::Deliverable::CodePatch { content } => (content.clone(), "CodePatch"),
                                };
                                log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Deliverable '{}' found in source task '{}'.", mapping.deliverable_key, source_node.id);

                                let task_input = TaskInput {
                                    id: Uuid::new_v4().to_string(), // Generate a new UUID for the input ID
                                    description: format!("Input from {}: {}", deliverable_type_name, &content[..std::cmp::min(content.len(), 100)]), // Simplified description
                                    data: Value::String(content), // Wrap content in Value::String
                                    source_deliverable_ids: vec![source_node.id.to_string()], // Use source task node ID as source deliverable ID
                                };
                                task_inputs.push(task_input);
                                log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Task input added for task '{}'.", task_node_clone.id);
                            }
                            None => {
                                log::warn!(
                                    "WARN: [TaskScheduler::run_scheduling_cycle] - Deliverable with key '{}' not found in outputs of source task {} for task {}. Marking input mapping as failed.",
                                    mapping.deliverable_key, mapping.source_task_id, task_node_clone.id
                                );
                                input_mapping_failed = true;
                                break;
                            }
                        }
                    }
                    None => {
                        log::warn!("WARN: [TaskScheduler::run_scheduling_cycle] - Source task node with ID {} not found for input mapping in task {}. Marking input mapping as failed.", mapping.source_task_id, task_node_clone.id);
                        input_mapping_failed = true;
                        break;
                    }
                }
            }

            if input_mapping_failed {
                log::error!("ERROR: [TaskScheduler::run_scheduling_cycle] - Input mapping failed for task '{}'. Marking as failed.", task_node_clone.id);
                { // This block defines the scope for graph_manager_guard
                    let graph_manager_guard = self.task_graph_manager.lock().await; // Lock is held for this block
                    log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Acquired TaskGraphManager lock for updating task status to Failed.");
                    let active_graphs_map_write_guard_opt = graph_manager_guard.get_task_graph_mut(&graph_id_str).await;
                    
                    if let Some(mut active_graphs_map_write_guard) = active_graphs_map_write_guard_opt { // active_graphs_map_write_guard is likely an RwLockWriteGuard
                        if let Some(graph) = active_graphs_map_write_guard.get_mut(&graph_id_str) {
                            if let Some(node) = graph.nodes.get_mut(&task_node_clone.id.to_string()) {
                                node.status = crate::common_types::TaskStatus::Failed; // Use full path
                                node.error_message = Some(format!("Input mapping failed. Could not resolve inputs based on mappings: {:?}", task_node_clone.task_spec.input_mappings));
                                log::info!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Task '{}' status updated to Failed.", task_node_clone.id);
                            } else {
                                log::error!("ERROR: [TaskScheduler::run_scheduling_cycle] - Failed to get mutable node '{}' to mark as Failed after input mapping failure.", task_node_clone.id);
                            }
                        } else {
                            log::error!("ERROR: [TaskScheduler::run_scheduling_cycle] - Failed to get mutable graph '{}' (from map) to mark task '{}' as Failed.", graph_id_str, task_node_clone.id);
                        }
                    } else {
                        log::error!("ERROR: [TaskScheduler::run_scheduling_cycle] - Failed to get task graph map (write) for graph '{}' to mark task '{}' as Failed.", graph_id_str, task_node_clone.id);
                    }
                }
                continue;
            }

            // Find an available agent
            log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Attempting to find available agent for task '{}'.", task_node_clone.id);
            let agent_opt = {
                let agent_manager_guard = self.agent_manager.lock().await;
                log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Acquired AgentManager lock.");
                let task_id_str = task_node_clone.id.to_string(); // Create a binding for the String
                agent_manager_guard.find_available_agent(
                    &task_id_str, // Use the bound String
                    task_node_clone.task_spec.required_agent_role.clone()
                ).await
            };

            if let Some(agent) = agent_opt {
                let agent_id = agent.id().await; // Await agent.id()
                log::info!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Found agent '{}' for task '{}'.", agent_id, task_node_clone.id);
                { // Scope for graph_manager lock to update task state
                    let graph_manager_guard = self.task_graph_manager.lock().await; // Lock is held for this block
                    log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Acquired TaskGraphManager lock for updating task status to Executing.");
                    let active_graphs_map_write_guard_opt = graph_manager_guard.get_task_graph_mut(&graph_id_str).await;
                    
                    if let Some(mut active_graphs_map_write_guard) = active_graphs_map_write_guard_opt { // active_graphs_map_write_guard is likely an RwLockWriteGuard
                        if let Some(graph) = active_graphs_map_write_guard.get_mut(&graph_id_str) {
                            if let Some(node) = graph.nodes.get_mut(&task_node_clone.id.to_string()) {
                                node.inputs = task_inputs;
                                node.status = crate::common_types::TaskStatus::Executing; // Use full path
                                node.assigned_agent_id = Some(agent_id.clone()); // Use awaited agent_id
                                log::info!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Task '{}' status updated to Executing and assigned to agent '{}'.", task_node_clone.id, agent_id);

                                let message = crate::common_types::Message {
                                    id: crate::common_types::generate_id(),
                                    sender_id: "orchestrator".to_string(),
                                    receiver_id: Some(agent_id.clone()), // Use awaited agent_id
                                    content: crate::common_types::MessageContent::TaskAssignment { task: node.clone() },
                                };

                                let bus_sender = self.communication_bus.get_sender();
                                log::debug!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Sending TaskAssignment message for task '{}' to agent '{}'.", task_node_clone.id, agent_id);
                                if let Err(e) = bus_sender.send(message) {
                                    log::error!("ERROR: [TaskScheduler::run_scheduling_cycle] - Failed to send TaskAssignment message for task '{}' to agent '{}': {}", task_node_clone.id, agent_id, e);
                                } else {
                                    log::info!("DEBUG: [TaskScheduler::run_scheduling_cycle] - TaskAssignment message sent for task '{}' to agent '{}'.", task_node_clone.id, agent_id);
                                }
                            } else {
                                log::error!("ERROR: [TaskScheduler::run_scheduling_cycle] - Task node '{}' not found in graph '{}' after finding agent.", task_node_clone.id, graph_id_str);
                            }
                        } else {
                            log::error!("ERROR: [TaskScheduler::run_scheduling_cycle] - Task graph '{}' not found (in map) after finding agent for task '{}'.", graph_id_str, task_node_clone.id);
                        }
                    } else {
                        log::error!("ERROR: [TaskScheduler::run_scheduling_cycle] - Task graph map not found (write) for graph '{}' after finding agent for task '{}'.", graph_id_str, task_node_clone.id);
                    }
                }
            } else {
                log::info!("DEBUG: [TaskScheduler::run_scheduling_cycle] - No available agent found for task '{}' with required role {:?}. Task remains ReadyToExecute.", task_node_clone.id, task_node_clone.task_spec.required_agent_role);
            }
        }

        log::info!("DEBUG: [TaskScheduler::run_scheduling_cycle] - Task scheduling cycle finished.");
        Ok(())
    }

    // The assign_task_node_to_agent method from CoreOrchestrator could potentially
    // be moved here or made accessible to the scheduler.
    // pub fn assign_task_node_to_agent(&self, task_node: TaskNode, agent_id: String) -> Result<(), broadcast::error::SendError<Message>> {
    //     println!("Orchestrator assigning task node {} to agent {}", task_node.id, agent_id);
    //     let message = Message {
    //         id: crate::common_types::generate_id(),
    //         sender_id: "orchestrator".to_string(), // Orchestrator's ID
    //         receiver_id: Some(agent_id.clone()), // Target the specific agent
    //         content: crate::common_types::MessageContent::TaskAssignment { task: task_node },
    //     };

    //     self.communication_bus.publish(message);
    //     Ok(()) // Return Ok(()) for success
    // }
}