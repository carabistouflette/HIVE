use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Deliverable {
    ResearchReport {
        content: String,
        sources: Vec<String>,
    },
    CodePatch {
        content: String,
    },
    // Add other deliverable types as needed
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum DeliverableStatus {
    #[default]
    Provisional,
    Validated,
    Rejected,
    Archived,
}

impl std::fmt::Display for DeliverableStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeliverableStatus::Provisional => write!(f, "Provisional"),
            DeliverableStatus::Validated => write!(f, "Validated"),
            DeliverableStatus::Rejected => write!(f, "Rejected"),
            DeliverableStatus::Archived => write!(f, "Archived"),
        }
    }
}


impl std::str::FromStr for DeliverableStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Provisional" => Ok(DeliverableStatus::Provisional),
            "Validated" => Ok(DeliverableStatus::Validated),
            "Rejected" => Ok(DeliverableStatus::Rejected),
            "Archived" => Ok(DeliverableStatus::Archived),
            _ => Err(anyhow::anyhow!("Unknown DeliverableStatus: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Sprint {
    pub id: Uuid,
    pub name: Option<String>,
    pub goal: String,
    pub status: SprintStatus,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub planned_tasks: Vec<Uuid>,
    pub completed_tasks: Vec<Uuid>,
    pub review_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SprintStatus {
    #[default]
    Planned,
    Active,
    Completed,
    Aborted,
}

impl std::fmt::Display for SprintStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SprintStatus::Planned => write!(f, "Planned"),
            SprintStatus::Active => write!(f, "Active"),
            SprintStatus::Completed => write!(f, "Completed"),
            SprintStatus::Aborted => write!(f, "Aborted"),
        }
    }
}

impl std::str::FromStr for SprintStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Planned" => Ok(SprintStatus::Planned),
            "Active" => Ok(SprintStatus::Active),
            "Completed" => Ok(SprintStatus::Completed),
            "Aborted" => Ok(SprintStatus::Aborted),
            _ => Err(anyhow::anyhow!("Unknown SprintStatus: {}", s)),
        }
    }
}