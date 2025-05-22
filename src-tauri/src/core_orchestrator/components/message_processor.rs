// src-tauri/src/core_orchestrator/components/message_processor.rs

use tokio::sync::{broadcast, mpsc, Mutex};
use crate::common_types::{Message, MessageContent};
use crate::agent_manager::AgentManager;
use crate::core_orchestrator::components::task_graph_manager::TaskGraphManager;
use crate::common_types::agent_defs::AgentRole;
use uuid::Uuid;
use std::sync::Arc;
use anyhow::Result;

/// Listens to the communication bus and forwards relevant messages for processing.
pub struct MessageProcessor {
    bus_receiver: broadcast::Receiver<Message>,
    bus_writer: broadcast::Sender<Message>,
    task_results_sender: mpsc::Sender<(String, MessageContent)>,
    agent_manager: Arc<Mutex<AgentManager>>,
    task_graph_manager: Arc<Mutex<TaskGraphManager>>,
}

impl MessageProcessor {
    /// Creates a new MessageProcessor.
    pub fn new(
        bus_receiver: broadcast::Receiver<Message>,
        bus_writer: broadcast::Sender<Message>,
        task_results_sender: mpsc::Sender<(String, MessageContent)>,
        agent_manager: Arc<Mutex<AgentManager>>,
        task_graph_manager: Arc<Mutex<TaskGraphManager>>,
    ) -> Self {
        MessageProcessor {
            bus_receiver,
            bus_writer,
            task_results_sender,
            agent_manager,
            task_graph_manager,
        }
    }

    /// Starts the listener task.
    pub fn start_listening(&mut self) -> Result<(), anyhow::Error> {
        let mut bus_receiver = self.bus_receiver.resubscribe();
        let task_results_sender = self.task_results_sender.clone();
        let bus_writer = self.bus_writer.clone();
        let agent_manager = self.agent_manager.clone();
        let task_graph_manager = self.task_graph_manager.clone();


        tokio::spawn(async move {
            println!("Orchestrator message processor started.");
            loop {
                match bus_receiver.recv().await {
                    Ok(message) => {
                        println!("Message Processor received message: {:?}", message);
                        match message.content {
                            MessageContent::AgentResponse(agent_response) => {
                                match agent_response {
                                    crate::common_types::message_defs::AgentResponse::TaskCompleted { task_id, agent_id, deliverable } => {
                                        if let Err(e) = task_results_sender.send((task_id.clone(), MessageContent::AgentResponse(crate::common_types::message_defs::AgentResponse::TaskCompleted { task_id, agent_id: agent_id.clone(), deliverable }))).await {
                                            eprintln!("Failed to send TaskCompleted message to results channel: {}", e);
                                        }
                                    }
                                    crate::common_types::message_defs::AgentResponse::TaskFailed { task_id, agent_id, error } => {
                                        if let Err(e) = task_results_sender.send((task_id.clone(), MessageContent::AgentResponse(crate::common_types::message_defs::AgentResponse::TaskFailed { task_id, agent_id: agent_id.clone(), error }))).await {
                                            eprintln!("Failed to send TaskFailed message to results channel: {}", e);
                                        }
                                    }
                                    _ => {
                                        // Handle other AgentResponse variants if necessary, or ignore
                                        println!("Message Processor received unhandled AgentResponse: {:?}", agent_response);
                                    }
                                }
                            }
                            MessageContent::RequestInformation(info_request) => {
                                println!("Message Processor received RequestInformation message: {:?}", info_request);
                                // Attempt to find a ResearcherAgent
                                let find_agent_result = agent_manager.lock().await.find_available_agent(&info_request.original_task_id, Some(AgentRole::Researcher)).await;
                                match find_agent_result {
                                    Some(target_agent) => {
                                        let target_agent_id = target_agent.id().await; // Await the async method
                                        println!("Found ResearcherAgent: {}. Forwarding RequestInformation.", target_agent_id);
                                        let forwarded_message = Message {
                                            id: Uuid::new_v4().to_string(),
                                            sender_id: "orchestrator".to_string(),
                                            receiver_id: Some(target_agent_id.clone()), // Use .clone() as target_agent_id is moved into Some()
                                            content: MessageContent::RequestInformation(info_request.clone()),
                                        };
                                        if let Err(e) = bus_writer.send(forwarded_message) {
                                            eprintln!("Failed to forward RequestInformation message: {}", e);
                                        }
                                    }
                                    None => {
                                        eprintln!("No ResearcherAgent found to handle RequestInformation: {:?}", info_request);
                                        println!("[DIAGNOSIS LOG] MessageProcessor: No ResearcherAgent found for task {}. Requesting agent: {}. An error response should be sent.", info_request.original_task_id, info_request.requesting_agent_id);
                                    }
                                }
                            }
                            MessageContent::ReturnInformation(info_response) => {
                                println!("Message Processor received ReturnInformation message: {:?}", info_response);
                                // Route the response back to the original requesting agent
                                let target_agent_id = info_response.original_requesting_agent_id.clone();
                                println!("Routing ReturnInformation to original requesting agent: {}", target_agent_id);
                                let forwarded_message = Message {
                                    id: Uuid::new_v4().to_string(),
                                    sender_id: "orchestrator".to_string(), // Or a specific orchestrator ID
                                    receiver_id: Some(target_agent_id.clone()),
                                    content: MessageContent::ReturnInformation(info_response.clone()),
                                };
                                if let Err(e) = bus_writer.send(forwarded_message) {
                                    eprintln!("Failed to route ReturnInformation message to {}: {}", target_agent_id, e);
                                }
                            }
                            MessageContent::DelegateSubTask(delegation_request) => {
                                println!("Message Processor received DelegateSubTask message: {:?}", delegation_request);
                                let parent_task_id = delegation_request.parent_task_id.clone();
                                let delegating_agent_id = delegation_request.delegating_agent_id.clone();

                                let send_delegation_failure = |error_msg: String| {
                                    let bus_writer_clone = bus_writer.clone();
                                    let delegating_agent_id_clone = delegating_agent_id.clone();
                                    let parent_task_id_clone = parent_task_id.clone();
                                    tokio::spawn(async move {
                                        let failure_message = Message {
                                            id: Uuid::new_v4().to_string(),
                                            sender_id: "orchestrator".to_string(),
                                            receiver_id: Some(delegating_agent_id_clone.clone()),
                                            content: MessageContent::AgentResponse(crate::common_types::message_defs::AgentResponse::TaskFailed {
                                                task_id: parent_task_id_clone,
                                                agent_id: delegating_agent_id_clone,
                                                error: error_msg,
                                            }),
                                        };
                                        if let Err(e) = bus_writer_clone.send(failure_message) {
                                            eprintln!("Failed to send delegation failure message: {}", e);
                                        }
                                    });
                                };

                                // Find the graph containing the parent task
                                let graph_id_option = task_graph_manager.lock().await.find_graph_id_for_task(&parent_task_id).await;
                                match graph_id_option {
                                    Some(graph_id) => {
                                      // Add the new subtask node to the graph
                                        let new_task_node_result = task_graph_manager.lock().await.add_task_node_to_graph(
                                            &graph_id,
                                            delegation_request.sub_task_spec.clone(),
                                            None,
                                            None
                                        ).await;

                                        match new_task_node_result {
                                            Ok(new_task_id) => {
                                                println!("Delegated subtask node created with ID: {} in graph {}", new_task_id, graph_id);

                                              // Add a dependency edge from the parent task to the new subtask
                                                let add_edge_result = task_graph_manager.lock().await.add_task_edge_to_graph(
                                                    &graph_id,
                                                    parent_task_id.clone(),
                                                    new_task_id.clone(),
                                                    None,
                                                    None,
                                                ).await;
                                                match add_edge_result {
                                                    Ok(_) => {
                                                        println!("Dependency edge added from {} to {} in graph {}", parent_task_id, new_task_id, graph_id);
                                                        // TODO: task_graph_manager.delegated_task_mapping is private. Add a public method if this mapping is needed.
                                                    }
                                                    Err(e) => {
                                                        let error_msg = format!("Failed to add dependency edge from {} to {} in graph {}: {}", parent_task_id, new_task_id, graph_id, e);
                                                        eprintln!("{}", error_msg);
                                                        send_delegation_failure(error_msg);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                let error_msg = format!("Failed to add delegated subtask node to graph {}: {}", graph_id, e);
                                                eprintln!("{}", error_msg);
                                                send_delegation_failure(error_msg);
                                            }
                                        }
                                    }
                                    None => {
                                        let error_msg = format!("Parent task with ID {} not found in any active task graph. Cannot delegate subtask.", parent_task_id);
                                        eprintln!("{}", error_msg);
                                        send_delegation_failure(error_msg);
                                    }
                                }
                            }
                            // Handle other message types if necessary, or ignore
                            _ => {}
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        eprintln!("Message Processor lagged behind by {} messages.", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        println!("Orchestrator bus channel closed. Message Processor shutting down.");
                        break;
                    }
                }
            }
        });
        Ok(())
    }
}