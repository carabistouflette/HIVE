use serde::{Deserialize, Serialize};

use crate::common_types::task_graph_defs::TaskNode;
use crate::common_types::mcp_defs::{MCPInput, MCPOutput};
use crate::common_types::sprint_defs::Deliverable;
use crate::common_types::task_defs::{TaskSpecification, SubTaskDefinition, SubTaskEdgeDefinition};
use crate::common_types::agent_defs::AgentRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub sender_id: String,
    pub receiver_id: Option<String>,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InformationRequest {
    pub original_task_id: String, // Task ID of the agent making the request
    pub requesting_agent_id: String,
    pub query: String, // The specific information being requested
    pub context: Option<String>, // Optional context for the query
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InformationResponse {
    pub original_task_id: String, // Task ID of the agent that made the request
    pub original_requesting_agent_id: String, // ID of the agent that originally requested the information
    pub responding_agent_id: String,
    pub request_query: String, // The original query for context
    pub response_data: String, // The information provided
    pub error: Option<String>, // If the request could not be fulfilled
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubTaskDelegationRequest {
    pub parent_task_id: String, // The ID of the task from which this subtask is delegated
    pub delegating_agent_id: String,
    pub sub_task_spec: TaskSpecification, // The specification for the new subtask
    // Potentially add fields for required role for the subtask, priority, etc.
    // For now, TaskSpecification should cover the sub_task_spec.required_agent_role
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageContent {
    TaskAssignment { task: TaskNode },
    TaskAcknowledgement { task_id: String, agent_id: String },
    StatusUpdate { agent_id: String, status: String, message: Option<String> },
    DataFragment { data: String },
    InvokeMCPRequest { request_id: String, input: MCPInput },
    InvokeMCPResponse { response: MCPOutput },
    AgentResponse(AgentResponse),
    RequestInformation(InformationRequest),
    ReturnInformation(InformationResponse),
    DelegateSubTask(SubTaskDelegationRequest),
    SubTasksGenerated {
        original_task_id: String,
        sub_tasks: Vec<SubTaskDefinition>,
        sub_task_edges: Vec<SubTaskEdgeDefinition>,
    },
    DelegatedTaskCompletedNotification {
        delegating_agent_id: String,
        completed_sub_task_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentResponse {
    TaskCompleted { task_id: String, agent_id: String, deliverable: Deliverable },
    TaskFailed { task_id: String, agent_id: String, error: String },
}