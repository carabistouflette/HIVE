use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TaskType {
    #[default]
    Generic,
    GenerateCode,
    Research,
    WriteContent,
    ValidateContent,
    DecomposeTask,
    // Add more task types as needed
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskType::Generic => write!(f, "Generic"),
            TaskType::GenerateCode => write!(f, "Generate Code"),
            TaskType::Research => write!(f, "Research"),
            TaskType::WriteContent => write!(f, "Write Content"),
            TaskType::ValidateContent => write!(f, "Validate Content"),
            TaskType::DecomposeTask => write!(f, "Decompose Task"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Task {
    pub id: Uuid,
    pub description: String,
    pub status: TaskStatus, // Modified to align with graph node states
    pub sprint_id: Option<Uuid>,
    pub dependencies: Vec<String>, // Keep existing dependencies for now, might be replaced by TaskEdge
}

impl std::str::FromStr for TaskType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Generic" => Ok(TaskType::Generic),
            "Generate Code" => Ok(TaskType::GenerateCode),
            "Research" => Ok(TaskType::Research),
            "Write Content" => Ok(TaskType::WriteContent),
            "Validate Content" => Ok(TaskType::ValidateContent),
            "Decompose Task" => Ok(TaskType::DecomposeTask),
            _ => Err(anyhow::anyhow!("Unknown TaskType: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TaskStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
    // New variants for TaskNode status
    PendingDependencies,
    ReadyToExecute,
    Executing,
    BlockedByError,
    AwaitingValidation,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "Pending"),
            TaskStatus::InProgress => write!(f, "In Progress"),
            TaskStatus::Completed => write!(f, "Completed"),
            TaskStatus::Failed => write!(f, "Failed"),
            TaskStatus::PendingDependencies => write!(f, "Pending Dependencies"),
            TaskStatus::ReadyToExecute => write!(f, "Ready To Execute"),
            TaskStatus::Executing => write!(f, "Executing"),
            TaskStatus::BlockedByError => write!(f, "Blocked By Error"),
            TaskStatus::AwaitingValidation => write!(f, "Awaiting Validation"),
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pending" => Ok(TaskStatus::Pending),
            "In Progress" => Ok(TaskStatus::InProgress),
            "Completed" => Ok(TaskStatus::Completed),
            "Failed" => Ok(TaskStatus::Failed),
            "Pending Dependencies" => Ok(TaskStatus::PendingDependencies),
            "Ready To Execute" => Ok(TaskStatus::ReadyToExecute),
            "Executing" => Ok(TaskStatus::Executing),
            "Blocked By Error" => Ok(TaskStatus::BlockedByError),
            "Awaiting Validation" => Ok(TaskStatus::AwaitingValidation),
            _ => Err(anyhow::anyhow!("Unknown TaskStatus: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputMapping {
    pub source_task_id: Uuid, // ID of the task providing the input
    pub deliverable_key: String, // A way to identify a specific part of the source task's output
    pub target_input_name: String, // How this input should be named/identified within the consuming task's context.
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TaskSpecification {
    pub name: String,
    pub description: String,
    pub required_role: crate::common_types::agent_defs::AgentRole, // Use full path
    pub input_mappings: Vec<InputMapping>,
    pub priority: Option<u8>,
    pub required_agent_role: Option<crate::common_types::agent_defs::AgentRole>, // Use full path
    pub context: Option<String>, // Added context field
    pub task_type: TaskType, // Added task_type field
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubTaskDefinition {
    pub title: String,
    pub description: String,
    // TaskSpecification will be used to create the actual TaskNode
    pub task_spec: TaskSpecification,
    // You might want to add a temporary ID or key for linking edges before full TaskNode creation
    pub temp_id: String, // e.g., use the title if unique, or generate a UUID string
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubTaskEdgeDefinition {
    pub from_subtask_temp_id: String, // Corresponds to SubTaskDefinition.temp_id
    pub to_subtask_temp_id: String,   // Corresponds to SubTaskDefinition.temp_id
    pub edge_type: crate::common_types::task_graph_defs::TaskEdgeType,      // Reuse existing TaskEdgeType
}
