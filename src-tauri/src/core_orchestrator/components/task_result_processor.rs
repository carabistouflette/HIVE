// src-tauri/src/core_orchestrator/components/task_result_processor.rs

use tokio::sync::mpsc;
use std::collections::HashMap; // Added HashMap import
use crate::common_types::{MessageContent, TaskStatus, Deliverable, SubTaskDefinition, SubTaskEdgeDefinition, TaskSpecification};
use crate::core_orchestrator::components::task_graph_manager::TaskGraphManager;
use chrono::{Utc, DateTime};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Processes task results received from agents and updates task graphs.
pub struct TaskResultProcessor {
    task_results_receiver: mpsc::Receiver<(String, MessageContent)>,
    // Standardize on Mutex
    task_graph_manager: Arc<Mutex<TaskGraphManager>>,
    agent_manager: Arc<Mutex<crate::agent_manager::AgentManager>>, // Changed to Arc<Mutex<AgentManager>>
    bus_sender: tokio::sync::broadcast::Sender<crate::common_types::Message>, // Added bus_sender to send messages
}

impl TaskResultProcessor {
    /// Creates a new TaskResultProcessor.
    pub fn new(
        task_results_receiver: mpsc::Receiver<(String, MessageContent)>,
        task_graph_manager: Arc<Mutex<TaskGraphManager>>,
        agent_manager: Arc<Mutex<crate::agent_manager::AgentManager>>,
        bus_sender: tokio::sync::broadcast::Sender<crate::common_types::Message>,
    ) -> Self {
        TaskResultProcessor {
            task_results_receiver,
            task_graph_manager,
            agent_manager,
            bus_sender,
        }
    }

    /// Starts the task result processing loop.
    pub async fn start_processing(&mut self) -> Result<()> {
        println!("Orchestrator task results processor started.");
        // Use the receiver from the struct
        while let Some((task_id, content)) = self.task_results_receiver.recv().await {
            println!("Orchestrator processing result for task: {}", task_id);
            let now: DateTime<Utc> = Utc::now();

            // Acquire write lock with proper async handling
            let graph_manager_arc = self.task_graph_manager.clone(); // Clone the Arc for use in helper functions

            // Find the task node in any active task graph
            let graph_id_option = {
                let graph_manager_guard = graph_manager_arc.lock().await;
                graph_manager_guard.find_graph_id_for_task(&task_id).await
            };

            if let Some(graph_id) = graph_id_option {
                match content {
                    MessageContent::AgentResponse(agent_response) => {
                        match agent_response {
                            crate::common_types::message_defs::AgentResponse::TaskCompleted { task_id: completed_task_id, agent_id: _, deliverable } => {
                                if task_id == completed_task_id {
                                    let agent_manager_clone = self.agent_manager.clone();
                                    let bus_sender_clone = self.bus_sender.clone();
                                    if let Err(e) = TaskResultProcessor::handle_task_completed_success(
                                        agent_manager_clone,
                                        bus_sender_clone,
                                        &task_id, vec![deliverable], now, &graph_id, graph_manager_arc.clone()
                                    ).await {
                                        eprintln!("Error in handle_task_completed_success for task {}: {}", task_id, e);
                                    }
                                } else {
                                    eprintln!("Received TaskCompleted for mismatched task_id: expected {}, received {}", task_id, completed_task_id);
                                }
                            },
                            crate::common_types::message_defs::AgentResponse::TaskFailed { task_id: failed_task_id, agent_id: _, error } => {
                                if task_id == failed_task_id {
                                    if let Err(e) = TaskResultProcessor::handle_task_failure(
                                        &task_id, error, false, now, &graph_id, graph_manager_arc.clone() // is_fatal is not part of AgentResponse::TaskFailed
                                    ).await {
                                        eprintln!("Error in handle_task_failure for task {}: {}", task_id, e);
                                    }
                                } else {
                                    eprintln!("Received TaskFailed for mismatched task_id: expected {}, received {}", task_id, failed_task_id);
                                }
                            },
                        }
                    },
                    MessageContent::SubTasksGenerated { original_task_id, sub_tasks, sub_task_edges } => {
                        if task_id == original_task_id {
                            if let Err(e) = TaskResultProcessor::handle_subtasks_generated(
                                &original_task_id, sub_tasks, sub_task_edges, now, &graph_id, graph_manager_arc.clone()
                            ).await {
                                eprintln!("Error processing subtasks for task {}: {}", original_task_id, e);
                            }
                        } else {
                            eprintln!("Received SubTasksGenerated for mismatched task_id: expected {}, received {}", task_id, original_task_id);
                        }
                    },
                    _ => {
                        eprintln!("Orchestrator received unexpected message content in results channel: {:?}", content);
                    },
                }
            } else {
                eprintln!("Orchestrator received result for unknown task: {}", task_id);
            }
        }
        Ok(())
    }

    /// Handles a successful TaskCompleted message content.
    async fn handle_task_completed_success(
        agent_manager_arc: Arc<Mutex<crate::agent_manager::AgentManager>>, // Changed to Arc<Mutex<AgentManager>>
        bus_sender_clone: tokio::sync::broadcast::Sender<crate::common_types::Message>,
        task_id: &str,
        outputs: Vec<Deliverable>,
        now: DateTime<Utc>,
        graph_id_str: &str,
        tgm_arc: Arc<Mutex<TaskGraphManager>> // Changed to Arc<Mutex<TaskGraphManager>>
    ) -> Result<()> {
        println!("Task {} completed successfully.", task_id);

        let mut tgm_instance = tgm_arc.lock().await;
        if let Some(mut graphs_map_guard) = tgm_instance.get_task_graph_mut(graph_id_str).await {
            if let Some(node) = graphs_map_guard.get_mut(graph_id_str).and_then(|g| g.nodes.get_mut(task_id)) {
                node.status = TaskStatus::Completed;
                node.outputs = outputs;
                node.updated_at = now;
                node.error_message = None;
                node.retry_count = 0;
            } else {
                eprintln!("Task node {} not found in graph {} after acquiring lock.", task_id, graph_id_str);
                return Err(anyhow::anyhow!("Task node not found for update"));
            }
            graphs_map_guard.get_mut(graph_id_str).map(|g| g.updated_at = now);
        } else {
            eprintln!("TaskGraph with ID {} not found for updating timestamp.", graph_id_str);
            return Err(anyhow::anyhow!("TaskGraph not found for update"));
        }

        if let Some(delegating_agent_id) = tgm_instance.remove_delegated_task_mapping(task_id.to_string()).await { // Assumes tgm_instance methods are async
            println!("Completed task {} was a delegated subtask. Notifying delegating agent {}.", task_id, delegating_agent_id);

            let agent_manager_locked = agent_manager_arc.lock().await; // Lock the agent manager
            if let Some(agent) = agent_manager_locked.get_agent(&delegating_agent_id) {
                if agent.get_status().await == crate::common_types::agent_defs::AgentStatus::WaitingForDelegatedTask {
                    println!("Delegating agent {} is waiting. Sending notification.", delegating_agent_id);
                    let notification_message = crate::common_types::Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        sender_id: "orchestrator".to_string(),
                        receiver_id: Some(delegating_agent_id.clone()),
                        content: crate::common_types::MessageContent::DelegatedTaskCompletedNotification {
                            delegating_agent_id: delegating_agent_id.clone(),
                            completed_sub_task_id: task_id.to_string(),
                        },
                    };

                    if let Err(e) = bus_sender_clone.send(notification_message) {
                        eprintln!("Failed to send DelegatedTaskCompletedNotification to agent {}: {}", delegating_agent_id, e);
                    } else {
                        println!("Sent DelegatedTaskCompletedNotification to agent {}.", delegating_agent_id);
                    }
                } else {
                    println!("Delegating agent {} is not in WaitingForDelegatedTask status ({:?}). Not sending notification.", delegating_agent_id, agent.get_status().await);
                }
            } else {
                eprintln!("Delegating agent {} not found.", delegating_agent_id);
            }
        }
        Ok(())
    }

    /// Handles task failure, including retry logic.
    async fn handle_task_failure( // No &mut self
        task_id: &str,
        error: String,
        is_fatal: bool,
        now: DateTime<Utc>,
        graph_id_str: &str,
        tgm_arc: Arc<Mutex<TaskGraphManager>> // Changed to Arc<Mutex<TaskGraphManager>>
    ) -> Result<()> {
        println!("Task {} failed with error: {}. Fatal: {}", task_id, error, is_fatal);

        let tgm_instance = tgm_arc.lock().await;
        if let Some(mut graphs_map_guard) = tgm_instance.get_task_graph_mut(graph_id_str).await {
            if let Some(node) = graphs_map_guard.get_mut(graph_id_str).and_then(|g| g.nodes.get_mut(task_id)) {
                node.updated_at = now;
                node.error_message = Some(error.clone());

                let max_retries = node.retry_policy.as_ref().map_or(0, |policy| policy.max_retries as u32);

                if is_fatal || node.retry_count >= max_retries {
                    println!("Task {} permanently failed after {} retries (max {}).", task_id, node.retry_count, max_retries);
                    node.status = TaskStatus::Failed;
                } else {
                    node.retry_count += 1;
                    println!("Task {} failed. Retrying (attempt {} of {}).", task_id, node.retry_count, max_retries);
                    node.status = TaskStatus::ReadyToExecute;
                }
            } else {
                eprintln!("Task node {} not found in graph {} after acquiring lock.", task_id, graph_id_str);
                return Err(anyhow::anyhow!("Task node not found for update"));
            }
            graphs_map_guard.get_mut(graph_id_str).map(|g| g.updated_at = now);
        } else {
            eprintln!("TaskGraph with ID {} not found for updating timestamp.", graph_id_str);
            return Err(anyhow::anyhow!("TaskGraph not found for update"));
        }
        Ok(())
    }

    /// Handles the SubTasksGenerated message content.
    async fn handle_subtasks_generated( // No &mut self
        original_task_id: &str,
        sub_tasks: Vec<SubTaskDefinition>,
        sub_task_edges: Vec<SubTaskEdgeDefinition>,
        now: DateTime<Utc>,
        graph_id: &str,
        tgm_arc: Arc<Mutex<TaskGraphManager>> // Changed to Arc<Mutex<TaskGraphManager>>
    ) -> Result<()> {
        println!("Task {} generated subtasks.", original_task_id);

        let mut tgm_instance = tgm_arc.lock().await;
        
        // Check if original node exists (it should, as we found its graph)
        if let Some(mut graphs_map_guard) = tgm_instance.get_task_graph_mut(graph_id).await {
            if !graphs_map_guard.get(graph_id).map_or(false, |g| g.nodes.contains_key(original_task_id)) {
                eprintln!("Original task node with ID {} not found in graph {}.", original_task_id, graph_id);
                return Err(anyhow::anyhow!("Original task node not found"));
            }
        } else {
            eprintln!("Graph with ID {} not found for original task node check.", graph_id);
            return Err(anyhow::anyhow!("Graph for original task node not found"));
        }

        let mut temp_id_to_node_id: HashMap<String, String> = HashMap::new();
        let mut decomposition_failed = false;
        let mut added_nodes_ids: Vec<String> = Vec::new();
        let mut added_edges_ids: Vec<String> = Vec::new();
        
        for sub_task_def in sub_tasks {
            let task_spec = TaskSpecification {
                name: sub_task_def.title.clone(),
                description: sub_task_def.description.clone(),
                required_role: sub_task_def.task_spec.required_role.clone(),
                input_mappings: sub_task_def.task_spec.input_mappings.clone(),
                priority: sub_task_def.task_spec.priority,
                required_agent_role: sub_task_def.task_spec.required_agent_role.clone(),
                context: sub_task_def.task_spec.context.clone(),
                task_type: sub_task_def.task_spec.task_type.clone(), // Add missing field
            };

            match tgm_instance.add_task_node_to_graph( // Call on tgm_instance
                graph_id, task_spec, None, None
            ).await {
                Ok(new_node_id_str) => {
                    temp_id_to_node_id.insert(sub_task_def.temp_id, new_node_id_str.clone());
                    added_nodes_ids.push(new_node_id_str);
                }
                Err(e) => {
                    eprintln!("Failed to add subtask node for original task {}: {}", original_task_id, e);
                    for node_id_str in &added_nodes_ids {
                        if let Err(remove_e) = tgm_instance.remove_task_node(graph_id, node_id_str).await { // await
                            eprintln!("Cleanup failed to remove node {}: {}", node_id_str, remove_e);
                        }
                    }
                    decomposition_failed = true;
                    break;
                }
            };
        }

        if !decomposition_failed {
            for edge_def in sub_task_edges {
                let from_node_id_str = match temp_id_to_node_id.get(&edge_def.from_subtask_temp_id) {
                    Some(id) => id.clone(),
                    None => {
                        eprintln!("From temp_id {} not found for edge.", edge_def.from_subtask_temp_id);
                        decomposition_failed = true; break;
                    }
                };
                let to_node_id_str = match temp_id_to_node_id.get(&edge_def.to_subtask_temp_id) {
                    Some(id) => id.clone(),
                    None => {
                        eprintln!("To temp_id {} not found for edge.", edge_def.to_subtask_temp_id);
                        decomposition_failed = true; break;
                    }
                };

                match tgm_instance.add_task_edge_to_graph( // Call on tgm_instance
                    graph_id,
                    from_node_id_str, // Pass String
                    to_node_id_str,   // Pass String
                    None, // SubTaskEdgeDefinition does not have a condition field
                    None
                ).await { // await
                    Ok(edge_id_str) => {
                        added_edges_ids.push(edge_id_str);
                    }
                    Err(e) => {
                        eprintln!("Failed to add subtask edge for original task {}: {}", original_task_id, e);
                        decomposition_failed = true; break;
                    }
                }
            }
        }
        
        // Cleanup if edge creation failed after some nodes/edges were added
        if decomposition_failed && !added_edges_ids.is_empty() { // Check if any edges were added before failure
            for edge_id_str in &added_edges_ids {
                if let Err(remove_e) = tgm_instance.remove_task_edge(graph_id, edge_id_str).await { // await
                    eprintln!("Cleanup failed to remove edge {}: {}", edge_id_str, remove_e);
                }
            }
            // Nodes are already cleaned up if node addition failed. If node addition succeeded but edge failed, clean nodes too.
            if !added_nodes_ids.is_empty() { // Ensure nodes were added before trying to clean them
                for node_id_str in &added_nodes_ids {
                     if let Err(remove_e) = tgm_instance.remove_task_node(graph_id, node_id_str).await { // await
                        eprintln!("Cleanup failed to remove node {}: {}", node_id_str, remove_e);
                    }
                }
            }
        }

        // Update original task node status
        // We need to operate on active_graphs_map_guard which holds the lock
        if let Some(mut graphs_map_guard) = tgm_instance.get_task_graph_mut(graph_id).await {
            if let Some(original_node_ref) = graphs_map_guard.get_mut(graph_id).and_then(|g| g.nodes.get_mut(original_task_id)) {
                if decomposition_failed {
                    eprintln!("Decomposition failed for task {}. Marking original task as Failed.", original_task_id);
                    original_node_ref.status = TaskStatus::Failed;
                    original_node_ref.updated_at = now;
                    original_node_ref.error_message = Some("Subtask decomposition failed".to_string());
                } else {
                    println!("Subtasks and edges added successfully for task {}. Marking original task as completed.", original_task_id);
                    original_node_ref.status = TaskStatus::Completed;
                    original_node_ref.updated_at = now;
                    original_node_ref.error_message = None;
                }
            } else {
                eprintln!("Original task node {} not found in graph {} during final update.", original_task_id, graph_id);
                 // This would be an internal error if previous checks passed.
            }
            graphs_map_guard.get_mut(graph_id).map(|g| g.updated_at = now); // Update graph timestamp
        } else {
            // This should not happen if graph_id was correctly found earlier.
            eprintln!("Graph {} not found during final update for task {}.", graph_id, original_task_id);
            return Err(anyhow::anyhow!("Graph not found during finalization of subtask generation"));
        }
        
        if decomposition_failed {
            return Err(anyhow::anyhow!("Failed to generate subtasks"));
        }

        Ok(())
    }
}