use async_trait::async_trait;
use anyhow::{Result, anyhow};
use reqwest;
use serde::{Deserialize, Serialize};
use std::env;

use super::{LLMProvider, LLMRequest, LLMResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenRouterMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
    // Include other fields if needed, like 'usage'
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
    // Include other fields if needed, like 'finish_reason'
}

#[derive(Debug)]
pub struct OpenRouterProvider {
    api_key: String,
    client: reqwest::Client,
}

impl OpenRouterProvider {
    pub fn new() -> Result<Self, anyhow::Error> {
        dotenv::dotenv().ok(); // Load environment variables from .env file
        let api_key = env::var("OPENROUTER_API_KEY")
            .map_err(|_| anyhow!("OPENROUTER_API_KEY not found in environment variables"))?;
        let client = reqwest::Client::new();
        Ok(OpenRouterProvider { api_key, client })
    }
}

#[async_trait]
impl LLMProvider for OpenRouterProvider {
    fn name(&self) -> String {
        "OpenRouter".to_string()
    }

    async fn generate(&self, request: LLMRequest) -> Result<LLMResponse, anyhow::Error> {
        let mut messages = Vec::new();

        if let Some(system_prompt) = &request.system_prompt {
            messages.push(OpenRouterMessage {
                role: "system".to_string(),
                content: system_prompt.clone(),
            });
        }

        messages.push(OpenRouterMessage {
            role: "user".to_string(),
            content: request.prompt.clone(),
        });

        let openrouter_request = OpenRouterRequest {
            model: request.model,
            messages,
        };

        let response = self.client.post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&openrouter_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(anyhow!("OpenRouter API error: Status {}, Body: {}", status, text));
        }

        let openrouter_response: OpenRouterResponse = response.json().await?;

        if let Some(choice) = openrouter_response.choices.into_iter().next() {
            Ok(LLMResponse {
                content: choice.message.content,
                // Map other fields if necessary
            })
        } else {
            Err(anyhow!("OpenRouter API returned no choices"))
        }
    }
}