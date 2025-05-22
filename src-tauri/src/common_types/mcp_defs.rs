use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::external_api_client::{LLMRequest, LLMResponse, LLMTokenCounts};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MCPDefinition {
    pub id: String,
    pub description: String,
    pub template: String,
    pub logic_module_path: Option<String>,
    pub default_llm_provider: Option<String>,
    // Add other fields as needed
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MCPInput {
    pub mcp_id: String,
    pub data: Value,
    pub context_overrides: Option<MCPContextOverrides>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MCPContextOverrides {
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub llm_parameters: Option<Value>,
    pub additional_context: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MCPOutput {
    pub request_id: String,
    pub mcp_id: String,
    pub status: MCPStatus,
    pub llm_request_details: Option<LLMRequest>,
    pub llm_response_details: Option<LLMResponse>,
    pub processed_content: Option<Value>,
    pub error_message: Option<String>,
    pub usage_metrics: Option<MCPUsageMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MCPStatus {
    Success,
    LLMError { provider_error: String },
    MCPProcessingError,
    ConfigurationError,
    // Add more statuses as needed
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MCPUsageMetrics {
    pub llm_token_counts: Option<LLMTokenCounts>,
    pub processing_time_ms: Option<u64>,
    pub llm_call_duration_ms: Option<u64>,
    // Add more metrics as needed
}