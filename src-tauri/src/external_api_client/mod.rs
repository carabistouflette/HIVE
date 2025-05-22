// Placeholder for external API client module
// Will include OpenRouter and Requesty clients

use serde::{Serialize, Deserialize};

pub mod providers;

use std::collections::HashMap;

use providers::LLMProvider;
pub use providers::LLMRequest;
pub use providers::LLMResponse;
use providers::openrouter_provider::OpenRouterProvider;
use providers::requesty_provider::RequestyProvider;

#[derive(Debug)] // Add Debug trait
pub struct ExternalApiClient {
    providers: HashMap<String, Box<dyn LLMProvider>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LLMTokenCounts {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

impl ExternalApiClient {
    pub fn new() -> Result<Self, anyhow::Error> {
        let mut providers: HashMap<String, Box<dyn LLMProvider>> = HashMap::new();

        // Initialize providers
        let openrouter = OpenRouterProvider::new()?;
        let requesty = RequestyProvider::new()?;

        providers.insert(openrouter.name(), Box::new(openrouter));
        providers.insert(requesty.name(), Box::new(requesty));

        Ok(ExternalApiClient { providers })
    }

    pub fn get_provider(&self, name: &str) -> Option<&Box<dyn LLMProvider>> {
        self.providers.get(name)
    }

    pub async fn call_llm(&self, provider_name: &str, request: LLMRequest) -> Result<LLMResponse, anyhow::Error> {
        match self.get_provider(provider_name) {
            Some(provider) => {
                println!("Calling LLM provider: {}", provider.name());
                provider.generate(request).await
            },
            None => Err(anyhow::anyhow!("Provider '{}' not found", provider_name)),
        }
    }
}