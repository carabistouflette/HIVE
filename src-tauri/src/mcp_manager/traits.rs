use crate::common_types::{MCPInput, MCPOutput}; // Adjust paths as needed
use async_trait::async_trait;
use anyhow::Result; // Or your project's preferred error type

// Assuming LLMRequest and LLMResponse are defined elsewhere, e.g., in common_types or external_api_client
// For now, let's assume they are in common_types for the trait definition to compile.
// If they are not, this will need adjustment later.
// Placeholder definitions if they don't exist:
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct LLMRequest { /* ... */ }
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct LLMResponse { /* ... */ }
use crate::external_api_client::LLMRequest; // Assuming LLMRequest is here
use crate::external_api_client::LLMResponse; // Assuming LLMResponse is here


#[async_trait]
pub trait MetaContextualPrompt: Send + Sync {
    fn id(&self) -> &'static str;
    async fn prepare_llm_request(&self, input: &MCPInput, context: &serde_json::Value) -> Result<LLMRequest>;
    async fn process_llm_response(&self, response: &LLMResponse, input: &MCPInput) -> Result<serde_json::Value>;
}