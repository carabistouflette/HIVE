pub mod core_orchestrator;
pub mod agent_manager;
pub mod agents;
pub mod communication_bus;
pub mod state_manager;
pub mod external_api_client;
pub mod tauri_bindings;
pub mod common_types;
pub mod mcp_manager;
pub mod persistence; // Add persistence module

use tauri::{async_runtime::spawn, Manager};
use std::sync::Arc; // Changed to std::sync::Arc
use tokio::sync::Mutex; // Changed to tokio::sync::Mutex
use crate::core_orchestrator::CoreOrchestrator;
use crate::communication_bus::CommunicationBus; // Import CommunicationBus
use crate::external_api_client::ExternalApiClient; // Import ExternalApiClient
use crate::mcp_manager::MCPManager; // Import MCPManager
use crate::common_types::{AgentConfig, AgentRole, TaskNode, TaskStatus}; // Re-added TaskNode and TaskStatus as they are used later
use uuid::Uuid;
use anyhow::anyhow; // Add this line

// Define the AppState struct to hold the orchestrator and DB connection
pub struct AppState {
    pub orchestrator: Arc<Mutex<CoreOrchestrator>>,
    pub db_connection: Arc<Mutex<rusqlite::Connection>>, // Add DB connection to state
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log::info!("Starting Tauri application setup.");
    tauri::Builder::default()
        .setup(move |app| {
            log::info!("Tauri application setup started.");
            // Initialize core components inside the setup closure
            let communication_bus = Arc::new(CommunicationBus::new());
            log::info!("Communication bus initialized.");

            let external_api_client = ExternalApiClient::new()
                .map_err(|e| {
                    log::error!("Failed to create ExternalApiClient: {}", e);
                    e
                })?;
            log::info!("External API client created.");

            let external_api_client_arc = Arc::new(external_api_client);

            // Establish database connection using app handle
            let app_handle = app.handle().clone();
            let db_connection = persistence::establish_connection()
                .map_err(|e| {
                    log::error!("Failed to establish database connection: {}", e);
                    e
                })?;
            log::info!("Database connection established.");
            let db_connection_arc = Arc::new(tokio::sync::Mutex::new(db_connection));

            // MCPManager::new is async, needs to be in an async block
            let mcp_manager_arc = Arc::new(tauri::async_runtime::block_on(async {
                MCPManager::new(Arc::clone(&external_api_client_arc))
                    .await
                    .map_err(|e| {
                        log::error!("Failed to create MCPManager: {}", e);
                        e
                    })
            })?);
            log::info!("MCPManager created.");

            let core_orchestrator = tauri::async_runtime::block_on(async {
                CoreOrchestrator::new(
                    Arc::clone(&communication_bus),
                    Arc::clone(&mcp_manager_arc),
                    Arc::clone(&db_connection_arc),
                ).await
                .map_err(|e| {
                    log::error!("Failed to create CoreOrchestrator: {}", e);
                    e
                })
            })?;
            log::info!("CoreOrchestrator created.");

            let core_orchestrator_arc = Arc::new(Mutex::new(core_orchestrator));

            // Manage the state
            app.manage(AppState {
                orchestrator: core_orchestrator_arc.clone(),
                db_connection: db_connection_arc.clone(),
            });
            log::info!("App state managed.");

            // Spawn the orchestration cycle task
            let orchestrator_run_arc: Arc<Mutex<CoreOrchestrator>> = Arc::clone(&core_orchestrator_arc);
            let _orchestrator_handle = spawn(async move { // Assign to a variable to avoid unused result warning
                log::info!("CoreOrchestrator run task started.");
                let orchestrator_instance = match Arc::try_unwrap(orchestrator_run_arc) {
                    Ok(mutex) => mutex.into_inner(),
                    Err(_arc) => {
                        log::error!("Failed to unwrap Arc for orchestrator run. Another Arc reference still exists.");
                        return Err(anyhow::anyhow!("Failed to unwrap Arc for orchestrator run"));
                    }
                };
                if let Err(e) = orchestrator_instance.run().await {
                    log::error!("CoreOrchestrator run task failed: {}", e);
                    return Err(e);
                }
                log::info!("CoreOrchestrator run task finished.");
                Ok(())
            });

            app.handle().plugin(tauri_plugin_fs::init())?;
            log::info!("Tauri FS plugin initialized.");

            env_logger::init();
            log::info!("env_logger initialized.");

            if cfg!(debug_assertions) {
                // Automatically invoke the task logic in debug mode after state is managed
                let app_handle = app.handle().clone();
            }
            log::info!("Tauri application setup finished successfully.");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::tauri_bindings::execute_agent_task
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    log::info!("Tauri application run complete.");
}
