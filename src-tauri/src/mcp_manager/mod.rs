use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::common_types::{MCPDefinition, MCPInput, MCPOutput, MCPStatus};
use serde_json::{self, Value};
use handlebars::Handlebars;
use uuid::Uuid;
use std::sync::Arc;
use crate::external_api_client::{ExternalApiClient, LLMRequest};

pub mod traits; // Declare the traits submodule

#[derive(Debug)] // Added Debug derive
pub struct MCPManager {
    definitions: HashMap<String, MCPDefinition>,
    handlebars: Handlebars<'static>,
    external_api_client: Arc<ExternalApiClient>,
}

impl MCPManager {
    pub async fn new(external_api_client: Arc<ExternalApiClient>) -> Result<Self> {
        println!("MCPManager created");
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true); // Configure handlebars
        let mut manager = MCPManager {
            definitions: HashMap::new(),
            handlebars,
            external_api_client,
        };
        manager.load_definitions("config/mcps/").await?;
        Ok(manager)
    }

    pub async fn load_definitions(&mut self, directory_path: &str) -> Result<()> {
        let path = Path::new(directory_path);

        if !path.exists() || !path.is_dir() {
            return Err(anyhow!("Directory not found or is not a directory: {}", directory_path));
        }

        println!("Loading MCP definitions from: {}", directory_path);

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().map_or(false, |ext| ext == "json" && path.to_string_lossy().ends_with(".mcp.json")) {
                let file_content = fs::read_to_string(&path)?;
                let definition: MCPDefinition = serde_json::from_str(&file_content)?;

                if definition.id.is_empty() {
                    eprintln!("Warning: Skipping MCP definition from {:?} due to empty ID.", path);
                    continue;
                }

                if self.definitions.contains_key(&definition.id) {
                    eprintln!("Warning: Skipping duplicate MCP definition with ID '{}' from {:?}.", definition.id, path);
                    continue;
                }

                println!("Loaded MCP definition: {}", definition.id);
                self.definitions.insert(definition.id.clone(), definition);
            }
        }

        Ok(())
    }

    pub async fn invoke_mcp(&self, input: MCPInput) -> Result<MCPOutput> {
        println!("Invoking MCP: {}", input.mcp_id);

        let request_id = Uuid::new_v4().to_string();

        // Retrieve Definition
        let mcp_definition = self.definitions.get(&input.mcp_id).ok_or_else(|| {
            anyhow!("MCP definition not found for ID: {}", input.mcp_id)
        })?;

        // Prepare LLMRequest (for template-based MCPs)
        // The template field is a String, based on common_types
        let prompt = self.handlebars.render_template(&mcp_definition.template, &input.data)?;

        // Determine LLM provider, model, and system prompt
        let context_overrides = input.context_overrides.as_ref();

        let provider_name = context_overrides.and_then(|c| c.llm_provider.clone())
            .or_else(|| mcp_definition.default_llm_provider.clone())
            .ok_or_else(|| anyhow!("LLM provider not specified in input or MCP definition for ID: {}", input.mcp_id))?;

        // LLMRequest requires a model name (String)
        let model_name = context_overrides.and_then(|c| c.llm_model.clone())
             .ok_or_else(|| anyhow!("LLM model not specified in input context overrides for ID: {}", input.mcp_id))?;

        // System prompt from additional_context if it's a string
        let system_prompt = context_overrides.and_then(|c| c.additional_context.as_ref())
            .and_then(|val| val.as_str().map(|s| s.to_string()));

        // Construct LLMRequest based on the definition in external_api_client/providers/mod.rs
        let llm_request = LLMRequest {
            model: model_name,
            prompt,
            system_prompt,
        };

        // Call ExternalApiClient
        let llm_call_result = self.external_api_client.call_llm(&provider_name, llm_request.clone()).await;

        // Handle the result and populate MCPOutput
        match llm_call_result {
            Ok(llm_response) => {
                Ok(MCPOutput {
                    request_id,
                    mcp_id: input.mcp_id,
                    status: MCPStatus::Success,
                    llm_request_details: Some(llm_request),
                    llm_response_details: Some(llm_response.clone()),
                    processed_content: Some(serde_json::json!(llm_response.content)),
                    error_message: None,
                    usage_metrics: None, // Populate usage_metrics if available from LLMResponse
                })
            }
            Err(e) => {
                Ok(MCPOutput {
                    request_id,
                    mcp_id: input.mcp_id,
                    status: MCPStatus::LLMError { provider_error: e.to_string() },
                    llm_request_details: Some(llm_request),
                    llm_response_details: None,
                    processed_content: None,
                    error_message: Some(e.to_string()),
                    usage_metrics: None,
                })
            }
        }
    }
}