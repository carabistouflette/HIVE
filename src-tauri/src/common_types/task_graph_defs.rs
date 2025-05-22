use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use serde_json::Value;

use crate::common_types::sprint_defs::Deliverable; // Import Deliverable
use crate::common_types::task_defs::{TaskStatus, TaskSpecification}; // Import TaskSpecification
use crate::common_types::agent_defs::AgentRole;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskEdgeType {
    Dependency,
    // Add other types as needed, e.g., DataFlow, ControlFlow
}
impl std::fmt::Display for TaskEdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskEdgeType::Dependency => write!(f, "Dependency"),
        }
    }
}

impl std::str::FromStr for TaskEdgeType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Dependency" => Ok(TaskEdgeType::Dependency),
            _ => Err(anyhow::anyhow!("Unknown TaskEdgeType: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskInput {
    pub id: String,
    pub description: String,
    pub data: Value,
    pub source_deliverable_ids: Vec<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskNode {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub task_spec: TaskSpecification, // Added task_spec field
    pub status: TaskStatus, // Using the enhanced TaskStatus enum
    pub agent_role_type: Option<AgentRole>,
    pub mcp_id: Option<String>,
    pub inputs: Vec<TaskInput>,
    pub outputs: Vec<Deliverable>, // Changed to Vec<Deliverable>
    pub retry_count: u32, // Added field to track current retries
    #[serde(default)] // Use default value (None) if retry_policy is null or missing
    pub retry_policy: Option<TaskRetryPolicy>,
    pub priority: u8,
    pub estimated_duration_ms: Option<u64>,
    pub actual_duration_ms: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub assigned_agent_id: Option<String>,
    pub error_message: Option<String>, // Added to store failure details
    pub sprint_id: Option<Uuid>, // Added sprint_id field
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)] // Add Default
pub struct TaskRetryPolicy {
    pub max_retries: u8,
    pub retry_delay_ms: u64,
    pub backoff_strategy: Option<BackoffStrategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackoffStrategy {
    Fixed,
    Exponential,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskEdge {
    pub id: Uuid,
    pub from_node_id: String, // Use String for node IDs as TaskNode uses Uuid which can be converted to String
    pub to_node_id: String, // Use String for node IDs
    pub condition: Option<String>,
    pub data_mapping: Option<HashMap<String, String>>,
    pub edge_type: TaskEdgeType, // Added TaskEdgeType
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskGraph {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub nodes: HashMap<String, TaskNode>, // Use String for key as TaskNode ID is Uuid converted to String
    pub edges: Vec<TaskEdge>,
    pub root_task_ids: Vec<String>, // Use String for root task IDs
    pub status: TaskGraphStatus,
    pub overall_goal: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskGraphStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Paused,
}

impl std::fmt::Display for TaskGraphStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskGraphStatus::Pending => write!(f, "Pending"),
            TaskGraphStatus::InProgress => write!(f, "In Progress"),
            TaskGraphStatus::Completed => write!(f, "Completed"),
            TaskGraphStatus::Failed => write!(f, "Failed"),
            TaskGraphStatus::Paused => write!(f, "Paused"),
        }
    }
}

impl std::str::FromStr for TaskGraphStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pending" => Ok(TaskGraphStatus::Pending),
            "In Progress" => Ok(TaskGraphStatus::InProgress),
            "Completed" => Ok(TaskGraphStatus::Completed),
            "Failed" => Ok(TaskGraphStatus::Failed),
            "Paused" => Ok(TaskGraphStatus::Paused),
            _ => Err(anyhow::anyhow!("Unknown TaskGraphStatus: {}", s)),
        }
    }
}