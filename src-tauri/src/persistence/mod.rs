use rusqlite::{Connection, params};
use uuid::Uuid;
use serde_json;
use chrono::{DateTime, Utc};
use anyhow::{Context, Result, anyhow};

use std::env;

use crate::common_types::task_graph_defs::{TaskNode, TaskEdge};
use crate::common_types::sprint_defs::Sprint;
use crate::common_types::task_defs::{TaskSpecification, TaskStatus};
use crate::common_types::sprint_defs::SprintStatus;

pub fn establish_connection() -> Result<Connection> {
    let app_data_dir = env::current_dir().context("Failed to get current directory")?;

    // Ensure the directory exists
    std::fs::create_dir_all(&app_data_dir).map_err(|e| rusqlite::Error::InvalidPath(std::path::PathBuf::from(format!("Failed to create app data directory: {}", e))))?;

    let db_path = app_data_dir.join("hive.sqlite");
    let conn = Connection::open(db_path).map_err(anyhow::Error::from)?;
    create_tables(&conn).map_err(anyhow::Error::from)?;
    Ok(conn)
}

fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            task_spec TEXT NOT NULL,
            status TEXT NOT NULL,
            agent_role_type TEXT,
            mcp_id TEXT,
            inputs TEXT NOT NULL,
            outputs TEXT NOT NULL,
            retry_count INTEGER NOT NULL,
            retry_policy TEXT,
            priority INTEGER NOT NULL,
            estimated_duration_ms INTEGER,
            actual_duration_ms INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            assigned_agent_id TEXT,
            error_message TEXT,
            sprint_id TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_dependencies (
            id TEXT PRIMARY KEY,
            source_task_id TEXT NOT NULL,
            target_task_id TEXT NOT NULL,
            edge_type TEXT NOT NULL,
            condition TEXT,
            data_mapping TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sprints (
            id TEXT PRIMARY KEY,
            name TEXT,
            goal TEXT NOT NULL,
            status TEXT NOT NULL,
            start_date TEXT,
            end_date TEXT,
            planned_tasks TEXT NOT NULL,
            completed_tasks TEXT NOT NULL,
            review_notes TEXT
        )",
        [],
    )?;
    Ok(())
}

// TaskNode persistence
pub fn save_task(conn: &Connection, task_node: &TaskNode) -> Result<()> {
    let task_spec_json = serde_json::to_string(&task_node.task_spec).unwrap_or_default();
    let inputs_json = serde_json::to_string(&task_node.inputs).unwrap_or_default();
    let outputs_json = serde_json::to_string(&task_node.outputs).unwrap_or_default();
    let retry_policy_json = serde_json::to_string(&task_node.retry_policy).unwrap_or_default();

    conn.execute(
        "INSERT OR REPLACE INTO tasks (id, name, description, task_spec, status, agent_role_type, mcp_id, inputs, outputs, retry_count, retry_policy, priority, estimated_duration_ms, actual_duration_ms, created_at, updated_at, assigned_agent_id, error_message, sprint_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![
            task_node.id.to_string(),
            task_node.name,
            task_node.description,
            task_spec_json,
            task_node.status.to_string(),
            task_node.agent_role_type.as_ref().map(|role| role.to_string()),
            task_node.mcp_id,
            inputs_json,
            outputs_json,
            task_node.retry_count,
            retry_policy_json,
            task_node.priority,
            task_node.estimated_duration_ms.map(|d| d as i64),
            task_node.actual_duration_ms.map(|d| d as i64),
            task_node.created_at.to_rfc3339(),
            task_node.updated_at.to_rfc3339(),
            task_node.assigned_agent_id,
            task_node.error_message,
            task_node.sprint_id.map(|id| id.to_string())
        ],
    )?;
    Ok(())
}

pub fn load_task(conn: &Connection, task_id: Uuid) -> Result<Option<TaskNode>> {
    let mut stmt = conn.prepare("SELECT id, name, description, task_spec, status, agent_role_type, mcp_id, inputs, outputs, retry_count, retry_policy, priority, estimated_duration_ms, actual_duration_ms, created_at, updated_at, assigned_agent_id, error_message, sprint_id FROM tasks WHERE id = ?1")?;
    let mut rows = stmt.query(params![task_id.to_string()])?;

    if let Some(row) = rows.next()? {
        let id_str: String = row.get(0)?;
        let name: String = row.get(1)?;
        let description: String = row.get(2)?;
        let task_spec_json: String = row.get(3)?;
        let status_str: String = row.get(4)?;
        let agent_role_type_str: Option<String> = row.get(5)?;
        let mcp_id: Option<String> = row.get(6)?;
        let inputs_json: String = row.get(7)?;
        let outputs_json: String = row.get(8)?;
        let retry_count: i32 = row.get(9)?;
        let retry_policy_json: Option<String> = row.get(10)?;
        let priority: i32 = row.get(11)?;
        let estimated_duration_ms: Option<i64> = row.get(12)?;
        let actual_duration_ms: Option<i64> = row.get(13)?;
        let created_at_str: String = row.get(14)?;
        let updated_at_str: String = row.get(15)?;
        let assigned_agent_id: Option<String> = row.get(16)?;
        let error_message: Option<String> = row.get(17)?;
        let sprint_id_str: Option<String> = row.get(18)?;

        let task_spec: TaskSpecification = serde_json::from_str(&task_spec_json).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize task_spec: {}", e)))?;
        let status: TaskStatus = status_str.parse().map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse TaskStatus: {}", e)))?;
        let agent_role_type = agent_role_type_str.map(|role| role.parse().map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse AgentRole: {}", e)))).transpose()?;
        let inputs = serde_json::from_str(&inputs_json).unwrap_or_default();
        let outputs = serde_json::from_str(&outputs_json).unwrap_or_default();
        let retry_policy = retry_policy_json.and_then(|json_str| serde_json::from_str(&json_str).ok());
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str).unwrap_or_default().with_timezone(&chrono::Utc);
        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str).unwrap_or_default().with_timezone(&chrono::Utc);
        let sprint_id = sprint_id_str.map(|id| Uuid::parse_str(&id).unwrap_or_default());


        Ok(Some(crate::common_types::task_graph_defs::TaskNode {
            id: Uuid::parse_str(&id_str).unwrap_or_default(),
            name,
            description,
            task_spec,
            status,
            agent_role_type,
            mcp_id,
            inputs,
            outputs,
            retry_count: retry_count as u32,
            retry_policy,
            priority: priority as u8,
            estimated_duration_ms: estimated_duration_ms.map(|d| d as u64),
            actual_duration_ms: actual_duration_ms.map(|d| d as u64),
            created_at,
            updated_at,
            assigned_agent_id,
            error_message,
            sprint_id,
        }))
    } else {
        Ok(None)
    }
}

pub fn load_all_tasks(conn: &Connection) -> Result<Vec<crate::common_types::task_graph_defs::TaskNode>> {
    let mut stmt = conn.prepare("SELECT id, name, description, task_spec, status, agent_role_type, mcp_id, inputs, outputs, retry_count, retry_policy, priority, estimated_duration_ms, actual_duration_ms, created_at, updated_at, assigned_agent_id, error_message, sprint_id FROM tasks").map_err(anyhow::Error::from)?;
    let task_nodes = stmt.query_map([], |row: &rusqlite::Row| -> rusqlite::Result<crate::common_types::task_graph_defs::TaskNode> {
        let id_str: String = row.get(0)?;
        let name: String = row.get(1)?;
        let description: String = row.get(2)?;
        let task_spec_json: String = row.get(3)?;
        let status_str: String = row.get(4)?;
        let agent_role_type_str: Option<String> = row.get(5)?;
        let mcp_id: Option<String> = row.get(6)?;
        let inputs_json: String = row.get(7)?;
        let outputs_json: String = row.get(8)?;
        let retry_count: i32 = row.get(9)?;
        let retry_policy_json: Option<String> = row.get(10)?;
        let priority: i32 = row.get(11)?;
        let estimated_duration_ms: Option<i64> = row.get(12)?;
        let actual_duration_ms: Option<i64> = row.get(13)?;
        let created_at_str: String = row.get(14)?;
        let updated_at_str: String = row.get(15)?;
        let assigned_agent_id: Option<String> = row.get(16)?;
        let error_message: Option<String> = row.get(17)?;
        let sprint_id_str: Option<String> = row.get(18)?;

        let task_spec: TaskSpecification = serde_json::from_str(task_spec_json.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize task_spec: {}", e)))?;
        let status: TaskStatus = status_str.parse().map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse TaskStatus: {}", e)))?;
        let agent_role_type = agent_role_type_str.map(|role| role.parse().map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse AgentRole: {}", e)))).transpose()?;
        let inputs = serde_json::from_str(inputs_json.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize inputs: {}", e)))?;
        let outputs = serde_json::from_str(outputs_json.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize outputs: {}", e)))?;
        let retry_policy = retry_policy_json
            .and_then(|json_str| {
                if json_str == "null" {
                    None
                } else {
                    serde_json::from_str(json_str.as_str())
                        .map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize retry_policy: {}", e)))
                        .ok() // Convert Result to Option, discarding error if deserialization fails
                }
            });
        let created_at = chrono::DateTime::parse_from_rfc3339(created_at_str.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse created_at: {}", e)))?.with_timezone(&chrono::Utc);
        let updated_at = chrono::DateTime::parse_from_rfc3339(updated_at_str.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse updated_at: {}", e)))?.with_timezone(&chrono::Utc);
        let sprint_id = sprint_id_str.map(|id| Uuid::parse_str(id.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse sprint_id: {}", e)))).transpose()?;
        Ok(crate::common_types::task_graph_defs::TaskNode {
            id: Uuid::parse_str(id_str.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse id: {}", e)))?,
            name,
            description,
            task_spec,
            status,
            agent_role_type,
            mcp_id,
            inputs,
            outputs,
            retry_count: retry_count as u32,
            retry_policy,
            priority: priority as u8,
            estimated_duration_ms: estimated_duration_ms.map(|d| d as u64),
            actual_duration_ms: actual_duration_ms.map(|d| d as u64),
            created_at,
            updated_at,
            assigned_agent_id,
            error_message,
            sprint_id,
        })
    })?
    .collect::<rusqlite::Result<Vec<crate::common_types::task_graph_defs::TaskNode>>>()
    .map_err(anyhow::Error::from)?;

    Ok(task_nodes)
}

// TaskEdge persistence
pub fn save_task_dependency(conn: &Connection, edge: &crate::common_types::task_graph_defs::TaskEdge) -> Result<()> {
    let condition_json = serde_json::to_string(&edge.condition).context("Failed to serialize condition")?;
    let data_mapping_json = serde_json::to_string(&edge.data_mapping).context("Failed to serialize data_mapping")?;

    conn.execute(
        "INSERT OR REPLACE INTO task_dependencies (id, source_task_id, target_task_id, edge_type, condition, data_mapping)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            edge.id.to_string(),
            edge.from_node_id,
            edge.to_node_id,
            edge.edge_type.to_string(), // Assuming TaskEdgeType implements Display
            condition_json,
            data_mapping_json
        ],
    ).map_err(anyhow::Error::from)?;
    Ok(())
}

pub fn load_all_task_dependencies(conn: &Connection) -> Result<Vec<crate::common_types::task_graph_defs::TaskEdge>> {
    let mut stmt = conn.prepare("SELECT id, source_task_id, target_task_id, edge_type, condition, data_mapping FROM task_dependencies").map_err(anyhow::Error::from)?;
    let task_edges = stmt.query_map([], |row: &rusqlite::Row| -> rusqlite::Result<crate::common_types::task_graph_defs::TaskEdge> {
        let id_str: String = row.get(0)?;
        let source_id_str: String = row.get(1)?;
        let target_id_str: String = row.get(2)?;
        let edge_type_str: String = row.get(3)?;
        let condition_json: String = row.get(4)?;
        let data_mapping_json: String = row.get(5)?;

        let condition = serde_json::from_str(condition_json.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize condition: {}", e)))?;
        let data_mapping = serde_json::from_str(data_mapping_json.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize data_mapping: {}", e)))?;

        Ok(crate::common_types::task_graph_defs::TaskEdge {
            id: Uuid::parse_str(id_str.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse id: {}", e)))?,
            from_node_id: source_id_str,
            to_node_id: target_id_str,
            condition,
            data_mapping,
            edge_type: edge_type_str.parse().map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse edge_type: {}", e)))?,
        })
    })?
    .collect::<rusqlite::Result<Vec<crate::common_types::task_graph_defs::TaskEdge>>>()
    .map_err(anyhow::Error::from)?;

    Ok(task_edges)
}

// Sprint persistence
pub fn save_sprint(conn: &Connection, sprint: &crate::common_types::sprint_defs::Sprint) -> Result<()> {
    let planned_tasks_json = serde_json::to_string(&sprint.planned_tasks).context("Failed to serialize planned_tasks")?;
    let completed_tasks_json = serde_json::to_string(&sprint.completed_tasks).context("Failed to serialize completed_tasks")?;

    conn.execute(
        "INSERT OR REPLACE INTO sprints (id, name, goal, status, start_date, end_date, planned_tasks, completed_tasks, review_notes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            sprint.id.to_string(),
            sprint.name,
            sprint.goal,
            sprint.status.to_string(),
            sprint.start_date.map(|d| d.to_rfc3339()),
            sprint.end_date.map(|d| d.to_rfc3339()),
            planned_tasks_json,
            completed_tasks_json,
            sprint.review_notes
        ],
    ).map_err(anyhow::Error::from)?;
    Ok(())
}

pub fn load_sprint(conn: &Connection, sprint_id: Uuid) -> Result<Option<crate::common_types::sprint_defs::Sprint>> {
    let mut stmt = conn.prepare("SELECT id, name, goal, status, start_date, end_date, planned_tasks, completed_tasks, review_notes FROM sprints WHERE id = ?1").map_err(anyhow::Error::from)?;
    let mut rows = stmt.query(params![sprint_id.to_string()]).map_err(anyhow::Error::from)?;

    if let Some(row) = rows.next().map_err(anyhow::Error::from)? {
        let id_str: String = row.get(0)?;
        let name: Option<String> = row.get(1)?;
        let goal: String = row.get(2)?;
        let status_str: String = row.get(3)?;
        let start_date_str: Option<String> = row.get(4)?;
        let end_date_str: Option<String> = row.get(5)?;
        let planned_tasks_json: String = row.get(6)?;
        let completed_tasks_json: String = row.get(7)?;
        let review_notes: Option<String> = row.get(8)?;

        let status: SprintStatus = status_str.parse().map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse SprintStatus: {}", e)))?;
        let start_date = start_date_str.map(|d| chrono::DateTime::parse_from_rfc3339(d.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse start_date: {}", e))).map(|dt| dt.with_timezone(&chrono::Utc))).transpose()?;
        let end_date = end_date_str.map(|d| chrono::DateTime::parse_from_rfc3339(d.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse end_date: {}", e))).map(|dt| dt.with_timezone(&chrono::Utc))).transpose()?;
        let planned_tasks = serde_json::from_str(planned_tasks_json.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize planned_tasks: {}", e)))?;
        let completed_tasks = serde_json::from_str(completed_tasks_json.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize completed_tasks: {}", e)))?;

        Ok(Some(crate::common_types::sprint_defs::Sprint {
            id: Uuid::parse_str(id_str.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse id: {}", e)))?,
            name,
            goal,
            status,
            start_date,
            end_date,
            planned_tasks,
            completed_tasks,
            review_notes,
        }))
    } else {
        Ok(None)
    }
}

pub fn load_all_sprints(conn: &Connection) -> Result<Vec<crate::common_types::sprint_defs::Sprint>> {
    let mut stmt = conn.prepare("SELECT id, name, goal, status, start_date, end_date, planned_tasks, completed_tasks, review_notes FROM sprints").map_err(anyhow::Error::from)?;
    let sprints = stmt.query_map([], |row: &rusqlite::Row| -> rusqlite::Result<crate::common_types::sprint_defs::Sprint> {
        let id_str: String = row.get(0)?;
        let name: Option<String> = row.get(1)?;
        let goal: String = row.get(2)?;
        let status_str: String = row.get(3)?;
        let start_date_str: Option<String> = row.get(4)?;
        let end_date_str: Option<String> = row.get(5)?;
        let planned_tasks_json: String = row.get(6)?;
        let completed_tasks_json: String = row.get(7)?;
        let review_notes: Option<String> = row.get(8)?;

        let status: SprintStatus = status_str.parse().map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse SprintStatus: {}", e)))?;
        let start_date = start_date_str.map(|d| chrono::DateTime::parse_from_rfc3339(d.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse start_date: {}", e))).map(|dt| dt.with_timezone(&chrono::Utc))).transpose()?;
        let end_date = end_date_str.map(|d| chrono::DateTime::parse_from_rfc3339(d.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse end_date: {}", e))).map(|dt| dt.with_timezone(&chrono::Utc))).transpose()?;
        let planned_tasks = serde_json::from_str(planned_tasks_json.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize planned_tasks: {}", e)))?;
        let completed_tasks = serde_json::from_str(completed_tasks_json.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to deserialize completed_tasks: {}", e)))?;

        Ok(crate::common_types::sprint_defs::Sprint {
            id: Uuid::parse_str(id_str.as_str()).map_err(|e| rusqlite::Error::InvalidParameterName(format!("Failed to parse id: {}", e)))?,
            name,
            goal,
            status,
            start_date,
            end_date,
            planned_tasks,
            completed_tasks,
            review_notes,
        })
    })?
    .collect::<rusqlite::Result<Vec<crate::common_types::sprint_defs::Sprint>>>()
    .map_err(anyhow::Error::from)?;

    Ok(sprints)
}