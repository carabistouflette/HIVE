use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AgentRole {
    #[default]
    Planner,
    Researcher,
    Writer,
    Coder,
    Validator,
    SimpleWorker,
    // Add more roles as needed
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRole::Planner => write!(f, "Planner"),
            AgentRole::Researcher => write!(f, "Researcher"),
            AgentRole::Writer => write!(f, "Writer"),
            AgentRole::Coder => write!(f, "Coder"),
            AgentRole::Validator => write!(f, "Validator"),
            AgentRole::SimpleWorker => write!(f, "Simple Worker"),
        }
    }
}

impl std::str::FromStr for AgentRole {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Planner" => Ok(AgentRole::Planner),
            "Researcher" => Ok(AgentRole::Researcher),
            "Writer" => Ok(AgentRole::Writer),
            "Coder" => Ok(AgentRole::Coder),
            "Validator" => Ok(AgentRole::Validator),
            "Simple Worker" => Ok(AgentRole::SimpleWorker),
            _ => Err(anyhow::anyhow!("Unknown AgentRole: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AgentStatus {
    #[default]
    Idle,
    Ready,
    Busy,
    WaitingForInformation, // Added status
    WaitingForDelegatedTask, // Added status for delegation
    Failed,
    TaskFailedRetryable,
    TaskFailedTerminal,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Idle => write!(f, "Idle"),
            AgentStatus::Ready => write!(f, "Ready"),
            AgentStatus::Busy => write!(f, "Busy"),
            AgentStatus::WaitingForInformation => write!(f, "Waiting For Information"),
            AgentStatus::WaitingForDelegatedTask => write!(f, "Waiting For Delegated Task"),
            AgentStatus::Failed => write!(f, "Failed"),
            AgentStatus::TaskFailedRetryable => write!(f, "Task Failed (Retryable)"),
            AgentStatus::TaskFailedTerminal => write!(f, "Task Failed (Terminal)"),
        }
    }
}

impl std::str::FromStr for AgentStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Idle" => Ok(AgentStatus::Idle),
            "Ready" => Ok(AgentStatus::Ready),
            "Busy" => Ok(AgentStatus::Busy),
            "Waiting For Information" => Ok(AgentStatus::WaitingForInformation),
            "Waiting For Delegated Task" => Ok(AgentStatus::WaitingForDelegatedTask),
            "Failed" => Ok(AgentStatus::Failed),
            "Task Failed (Retryable)" => Ok(AgentStatus::TaskFailedRetryable),
            "Task Failed (Terminal)" => Ok(AgentStatus::TaskFailedTerminal),
            _ => Err(anyhow::anyhow!("Unknown AgentStatus: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AgentConfig {
    pub id: String,
    pub role: AgentRole,
    pub llm_model: Option<String>,
    pub llm_provider_name: Option<String>,
    pub specialized_config: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCapabilities {
    pub can_research: bool,
    pub can_write: bool,
    pub can_plan: bool,
    pub can_code: bool,
    pub can_design: bool,
    pub can_test: bool,
    pub can_debug: bool,
    pub can_architect: bool,
    pub can_manage_sprint: bool,
    pub can_use_tool: bool,
}