use async_trait::async_trait;
use anyhow::{Result, anyhow};
use reqwest;
use serde::{Deserialize, Serialize};
use std::env;

use super::{LLMProvider, LLMRequest, LLMResponse};

// Assuming Requesty uses a similar chat completion structure to OpenAI/OpenRouter
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestyRequest {
    model: String,
    messages: Vec<RequestyMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestyMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestyResponse {
    choices: Vec<RequestyChoice>,
    // Include other fields if needed, like 'usage'
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestyChoice {
    message: RequestyMessage,
    // Include other fields if needed, like 'finish_reason'
}

#[derive(Debug)]
pub struct RequestyProvider {
    api_key: String,
    client: reqwest::Client,
    api_endpoint: String, // Added for flexibility
}

impl RequestyProvider {
    pub fn new() -> Result<Self, anyhow::Error> {
        dotenv::dotenv().ok(); // Load environment variables from .env file
        let api_key = env::var("REQUESTY_API_KEY")
            .map_err(|_| anyhow!("REQUESTY_API_KEY not found in environment variables"))?;
        // Assuming a default endpoint, but could also load from env var
        let api_endpoint = env::var("REQUESTY_API_ENDPOINT")
            .unwrap_or_else(|_| "https://api.requesty.com/v1/chat/completions".to_string()); // Replace with actual endpoint if known

        let client = reqwest::Client::new();
        Ok(RequestyProvider { api_key, client, api_endpoint })
    }
}

#[async_trait]
impl LLMProvider for RequestyProvider {
    fn name(&self) -> String {
        "Requesty".to_string()
    }

    async fn generate(&self, request: LLMRequest) -> Result<LLMResponse, anyhow::Error> {
        let mut messages = Vec::new();

        if let Some(system_prompt) = &request.system_prompt {
            messages.push(RequestyMessage {
                role: "system".to_string(),
                content: system_prompt.clone(),
            });
        }

        messages.push(RequestyMessage {
            role: "user".to_string(),
            content: request.prompt.clone(),
        });

        let requesty_request = RequestyRequest {
            model: request.model,
            messages,
        };

        let response = self.client.post(&self.api_endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&requesty_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(anyhow!("Requesty API error: Status {}, Body: {}", status, text));
        }

        let requesty_response: RequestyResponse = response.json().await?;

        if let Some(choice) = requesty_response.choices.into_iter().next() {
            Ok(LLMResponse {
                content: choice.message.content,
                // Map other fields if necessary
            })
        } else {
            Err(anyhow!("Requesty API returned no choices"))
        }
    }
}