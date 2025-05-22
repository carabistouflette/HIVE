use async_trait::async_trait;

pub mod openrouter_provider;
pub mod requesty_provider;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LLMRequest {
    pub model: String,
    pub prompt: String,
    // Autres paramètres comme temperature, max_tokens, etc.
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LLMResponse {
    pub content: String,
    // Autres informations comme usage des tokens, etc.
}

#[async_trait]
pub trait LLMProvider: Send + Sync + std::fmt::Debug {
    fn name(&self) -> String;
    async fn generate(&self, request: LLMRequest) -> Result<LLMResponse, anyhow::Error>;
    // Potentiellement d'autres méthodes comme list_models, etc.
}