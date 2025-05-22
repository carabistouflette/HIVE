// src-tauri/src/core_orchestrator/components/task_graph_manager.rs

use std::collections::HashMap;
use crate::common_types::{TaskGraph, TaskNode, TaskEdge, Sprint, TaskSpecification};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use anyhow::Result;
use uuid::Uuid;
use chrono::{Utc, DateTime};
use crate::persistence::{self};

/// Manages the lifecycle and operations of TaskGraphs.
#[derive(Debug)] // Add Debug trait
pub struct TaskGraphManager {
    active_task_graphs: RwLock<HashMap<String, TaskGraph>>,
    delegated_task_mapping: Mutex<HashMap<String, String>>, // Maps delegated task ID to delegating agent ID
    sprints: RwLock<HashMap<Uuid, Sprint>>,
    db_connection: Arc<Mutex<rusqlite::Connection>>,
}

impl TaskGraphManager {
    /// Creates a new TaskGraphManager.
    pub async fn new(db_connection: Arc<Mutex<rusqlite::Connection>>) -> Result<Self> {
        let manager = TaskGraphManager {
            active_task_graphs: RwLock::new(HashMap::new()),
            delegated_task_mapping: Mutex::new(HashMap::new()),
            sprints: RwLock::new(HashMap::new()),
            db_connection: db_connection.clone(),
        };

        {
            let conn = manager.db_connection.lock().await;
            manager.load_state_from_db(&*conn).await?;
        }
        
        Ok(manager)
    }

    async fn load_state_from_db(&self, conn: &rusqlite::Connection) -> Result<()> {
        let tasks = persistence::load_all_tasks(conn)?;
        let edges = persistence::load_all_task_dependencies(conn)?;
        let loaded_sprints = persistence::load_all_sprints(conn)?;
        
        // First load all data, then update state in one go
        let mut new_graphs = HashMap::new();
        let mut new_sprints = HashMap::new();
        
        for task in tasks {
            let graph_id = task.sprint_id.map(|id| id.to_string()).unwrap_or_else(|| "default_graph".to_string());
            let graph = new_graphs.entry(graph_id.clone())
                .or_insert_with(|| TaskGraph {
                    id: Uuid::parse_str(&graph_id).unwrap_or_default(),
                    name: "Loaded Graph".to_string(),
                    description: "Graph loaded from DB".to_string(),
                    overall_goal: "Manage tasks loaded from DB".to_string(),
                    nodes: HashMap::new(),
                    edges: Vec::new(),
                    root_task_ids: Vec::new(),
                    status: crate::common_types::TaskGraphStatus::Pending,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                });
            graph.nodes.insert(task.id.to_string(), task);
        }

        for edge in edges {
            if let Some(graph) = new_graphs.get_mut(&edge.from_node_id.to_string()) {
                graph.edges.push(edge);
            }
        }

        for sprint in loaded_sprints {
            new_sprints.insert(sprint.id, sprint);
        }

        // Update state atomically
        {
            let mut active_graphs = self.active_task_graphs.write().await;
            *active_graphs = new_graphs;
        }
        {
            let mut sprints = self.sprints.write().await;
            *sprints = new_sprints;
        }

        // TODO: Reconstruct root_task_ids and potentially graph structure from loaded data

        Ok(())
    }


    /// Creates a new TaskGraph with the given details and stores it.
    pub async fn create_task_graph(&self, name: String, description: String, overall_goal: String) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now: DateTime<Utc> = Utc::now();

        let new_graph = TaskGraph {
            id: Uuid::parse_str(&id)?,
            name,
            description,
            overall_goal,
            nodes: HashMap::new(),
            edges: Vec::new(),
            root_task_ids: Vec::new(),
            status: crate::common_types::TaskGraphStatus::Pending,
            created_at: now,
            updated_at: now,
        };

        {
            let mut active_graphs = self.active_task_graphs.write().await;
            active_graphs.insert(id.clone(), new_graph);
        }

        // No direct persistence for TaskGraph itself in this schema, only its components
        Ok(id)
    }

    /// Retrieves a mutable reference to all active TaskGraphs
    pub async fn get_active_graphs_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, HashMap<String, TaskGraph>> {
        self.active_task_graphs.write().await
    }

    /// Retrieves a mutable reference to a specific TaskGraph by its ID
    pub async fn get_task_graph_mut(&self, graph_id: &str) -> Option<tokio::sync::RwLockWriteGuard<'_, HashMap<String, TaskGraph>>> {
        let graphs = self.active_task_graphs.write().await;
        graphs.contains_key(graph_id).then(|| graphs)
    }

    pub async fn remove_task_node(&self, graph_id: &str, node_id: &str) -> Result<()> {
        let mut graphs = self.active_task_graphs.write().await;
        if let Some(graph) = graphs.get_mut(graph_id) {
            graph.nodes.remove(node_id);
            graph.updated_at = Utc::now();
        }
        Ok(())
    }

    pub async fn remove_task_edge(&self, graph_id: &str, edge_id: &str) -> Result<()> {
        let mut graphs = self.active_task_graphs.write().await;
        if let Some(graph) = graphs.get_mut(graph_id) {
            graph.edges.retain(|e| e.id.to_string() != edge_id);
            graph.updated_at = Utc::now();
        }
        Ok(())
    }

    pub async fn get_active_graphs(&self) -> tokio::sync::RwLockReadGuard<'_, HashMap<String, TaskGraph>> {
        self.active_task_graphs.read().await
    }

    pub async fn remove_delegated_task_mapping(&self, task_id: String) -> Option<String> {
        let mut mapping = self.delegated_task_mapping.lock().await;
        mapping.remove(&task_id)
    }

    /// Retrieves an immutable reference to a TaskGraph by its ID.
    pub async fn get_task_graph(&self, graph_id: &str) -> Option<tokio::sync::RwLockReadGuard<'_, HashMap<String, TaskGraph>>> {
        let graphs = self.active_task_graphs.read().await;
        if graphs.contains_key(graph_id) {
            Some(graphs)
        } else {
            None
        }
    }

    /// Adds a new TaskNode to a specified TaskGraph.
    pub async fn add_task_node_to_graph(
        &self,
        graph_id: &str,
        task_spec: TaskSpecification,
        sprint_id: Option<Uuid>,
        estimated_duration_ms: Option<u64>,
    ) -> Result<String> {
        let node_id = Uuid::new_v4();
        let now: DateTime<Utc> = Utc::now();

        let new_node = TaskNode {
            id: node_id,
            name: task_spec.name.clone(),
            description: task_spec.description.clone(),
            task_spec: task_spec.clone(),
            status: crate::common_types::task_defs::TaskStatus::PendingDependencies,
            agent_role_type: task_spec.required_agent_role.clone(),
            mcp_id: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            retry_count: 0,
            retry_policy: None,
            priority: task_spec.priority.clone().unwrap_or(0),
            estimated_duration_ms,
            actual_duration_ms: None,
            created_at: now,
            updated_at: now,
            assigned_agent_id: None,
            error_message: None,
            sprint_id,
        };

        // Persist first
        {
            let conn = self.db_connection.lock().await;
            persistence::save_task(&conn, &new_node)?;
        }

        // Then update in-memory state
        {
            let mut graphs = self.active_task_graphs.write().await;
            let graph = graphs.get_mut(graph_id)
                .ok_or_else(|| anyhow::anyhow!("TaskGraph with ID {} not found", graph_id))?;
            
            graph.nodes.insert(new_node.id.to_string(), new_node);
            graph.updated_at = Utc::now();
        }

        Ok(node_id.to_string())
    }

    /// Adds a new TaskEdge to a specified TaskGraph.
    pub async fn add_task_edge_to_graph(
        &self,
        graph_id: &str,
        from_node_id: String,
        to_node_id: String,
        condition: Option<String>,
        data_mapping: Option<HashMap<String, String>>
    ) -> Result<String> {
        let mut graphs = self.active_task_graphs.write().await;
        let graph = graphs.get_mut(graph_id)
            .ok_or_else(|| anyhow::anyhow!("TaskGraph with ID {} not found", graph_id))?;

        // Validate that from_node_id and to_node_id exist
        if !graph.nodes.contains_key(&from_node_id) {
            return Err(anyhow::anyhow!("From node with ID {} not found in graph {}", from_node_id, graph_id));
        }
        if !graph.nodes.contains_key(&to_node_id) {
            return Err(anyhow::anyhow!("To node with ID {} not found in graph {}", to_node_id, graph_id));
        }

        let edge_id = Uuid::new_v4();

        let new_edge = TaskEdge {
            id: edge_id, // Use Uuid directly
            from_node_id,
            to_node_id,
            condition,
            data_mapping,
            edge_type: crate::common_types::task_graph_defs::TaskEdgeType::Dependency, // Corrected edge_type path
        };

        {
            let conn = self.db_connection.lock().await;
            let save_result = persistence::save_task_dependency(&conn, &new_edge);
            save_result?;
        }

        graph.edges.push(new_edge);
        graph.updated_at = Utc::now();

        Ok(edge_id.to_string())
    }

    /// Finds the ID of the TaskGraph containing the given task ID.
    pub async fn find_graph_id_for_task(&self, task_id: &str) -> Option<String> {
        let graphs = self.active_task_graphs.read().await;
        for (graph_id, graph) in graphs.iter() {
            if graph.nodes.contains_key(task_id) {
                return Some(graph_id.clone());
            }
        }
        None
    }

    /// Creates a new Sprint with the given details and stores it.
    pub async fn create_sprint(&self, name: Option<String>, goal: String) -> Result<Uuid> {
        let id = Uuid::new_v4();

        let new_sprint = Sprint {
            id,
            name,
            goal,
            status: crate::common_types::sprint_defs::SprintStatus::Planned,
            start_date: None,
            end_date: None,
            planned_tasks: Vec::new(),
            completed_tasks: Vec::<Uuid>::new(),
            review_notes: None,
        };

        // Persist first
        {
            let conn = self.db_connection.lock().await;
            persistence::save_sprint(&conn, &new_sprint)?;
        }

        // Then update in-memory state
        {
            let mut sprints = self.sprints.write().await;
            sprints.insert(id, new_sprint);
        }

        Ok(id)
    }

    /// Retrieves an immutable reference to the sprints map.
    pub async fn get_sprints(&self) -> tokio::sync::RwLockReadGuard<'_, HashMap<Uuid, Sprint>> {
        self.sprints.read().await
    }

    /// Retrieves a mutable reference to the sprints map.
    pub async fn get_sprints_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, HashMap<Uuid, Sprint>> {
        self.sprints.write().await
    }

    /// Deletes a Sprint by its ID.
    pub async fn delete_sprint(&self, sprint_id: &Uuid) -> Option<Sprint> {
        let mut sprints = self.sprints.write().await;
        sprints.remove(sprint_id)
    }

    /// Starts a planned Sprint.
    pub async fn start_sprint(&self, sprint_id: &Uuid) -> Result<()> {
        let sprint = {
            let sprints = self.sprints.read().await;
            sprints.get(sprint_id)
                .ok_or_else(|| anyhow::anyhow!("Sprint with ID {} not found", sprint_id))?
                .clone()
        };

        if sprint.status != crate::common_types::sprint_defs::SprintStatus::Planned {
            return Err(anyhow::anyhow!("Sprint with ID {} is not in Planned status", sprint_id));
        }

        let mut updated_sprint = sprint;
        updated_sprint.status = crate::common_types::sprint_defs::SprintStatus::Active;
        updated_sprint.start_date = Some(Utc::now());

        // Persist first
        {
            let conn = self.db_connection.lock().await;
            persistence::save_sprint(&conn, &updated_sprint)?;
        }

        // Then update in-memory state
        {
            let mut sprints = self.sprints.write().await;
            sprints.insert(*sprint_id, updated_sprint);
        }

        Ok(())
    }

    /// Completes an active Sprint.
    pub async fn complete_sprint(&self, sprint_id: &Uuid) -> Result<()> {
        let sprint = {
            let sprints = self.sprints.read().await;
            sprints.get(sprint_id)
                .ok_or_else(|| anyhow::anyhow!("Sprint with ID {} not found", sprint_id))?
                .clone()
        };

        if sprint.status != crate::common_types::sprint_defs::SprintStatus::Active {
            return Err(anyhow::anyhow!("Sprint with ID {} is not in Active status", sprint_id));
        }

        let mut updated_sprint = sprint;
        updated_sprint.status = crate::common_types::sprint_defs::SprintStatus::Completed;
        updated_sprint.end_date = Some(Utc::now());

        // Persist first
        {
            let conn = self.db_connection.lock().await;
            persistence::save_sprint(&conn, &updated_sprint)?;
        }

        // Then update in-memory state
        {
            let mut sprints = self.sprints.write().await;
            sprints.insert(*sprint_id, updated_sprint);
        }

        Ok(())
    }
}