use serde::{Deserialize, Serialize};
use reqwest::blocking::Client;
use anyhow::{Result, anyhow};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub enum LlmApiMode {
    OpenAiChat,
    KoboldCpp,
}

#[derive(Clone, Debug)]
pub struct LlmConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub api_mode: LlmApiMode,
}

#[derive(Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
}

#[derive(Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<Choice>,
}

#[derive(Deserialize)]
pub struct Choice {
    pub message: ChatMessageResponse,
}

#[derive(Deserialize)]
pub struct ChatMessageResponse {
    pub content: String,
}

#[derive(Serialize)]
pub struct KoboldGenerateRequest {
    pub prompt: String,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
}

#[derive(Deserialize)]
pub struct KoboldGenerateResponse {
    pub results: Vec<KoboldGenerateResult>,
}

#[derive(Deserialize)]
pub struct KoboldGenerateResult {
    pub text: String,
}

pub fn call_llm(prompt: String, cfg: &LlmConfig) -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    match cfg.api_mode {
        LlmApiMode::OpenAiChat => {
            let req = ChatCompletionRequest {
                model: cfg.model.clone(),
                temperature: 0.7,
                messages: vec![
                    ChatMessage {
                        role: "system".into(),
                        content: prompt,
                    }
                ],
            };

            let url = join_url(&cfg.base_url, "chat/completions");
            let mut request = client.post(url).json(&req);
            if let Some(key) = cfg.api_key.as_ref().filter(|k| !k.trim().is_empty()) {
                request = request.bearer_auth(key);
            }

            let resp = request.send()?.json::<ChatCompletionResponse>()?;
            let first = resp.choices.get(0).ok_or_else(|| anyhow!("LLM returned no choices"))?;
            Ok(first.message.content.clone())
        }
        LlmApiMode::KoboldCpp => {
            let req = KoboldGenerateRequest {
                prompt,
                temperature: 0.7,
                max_length: None,
            };
            let url = join_url(&cfg.base_url, "api/v1/generate");
            let resp = client.post(url).json(&req).send()?.json::<KoboldGenerateResponse>()?;
            let first = resp
                .results
                .get(0)
                .ok_or_else(|| anyhow!("KoboldCpp returned no results"))?;
            Ok(first.text.clone())
        }
    }
}

pub fn test_connection(cfg: &LlmConfig) -> Result<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    match cfg.api_mode {
        LlmApiMode::OpenAiChat => {
            let url = join_url(&cfg.base_url, "models");
            let mut request = client.get(url);
            if let Some(key) = cfg.api_key.as_ref().filter(|k| !k.trim().is_empty()) {
                request = request.bearer_auth(key);
            }
            let resp: serde_json::Value = request.send()?.json()?;

            Ok(format!(
                "Connected ({} models available)",
                resp["data"].as_array().map(|a| a.len()).unwrap_or(0)
            ))
        }
        LlmApiMode::KoboldCpp => {
            let url = join_url(&cfg.base_url, "api/v1/model");
            let resp: serde_json::Value = client.get(url).send()?.json()?;
            let name = resp["result"]
                .as_str()
                .unwrap_or("KoboldCpp");
            Ok(format!("Connected ({})", name))
        }
    }
}

pub fn abort_generation(cfg: &LlmConfig) -> Result<()> {
    match cfg.api_mode {
        LlmApiMode::OpenAiChat => Ok(()),
        LlmApiMode::KoboldCpp => {
            let client = Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?;
            let url = join_url(&cfg.base_url, "api/extra/abort");
            let _ = client.post(url).send()?;
            Ok(())
        }
    }
}

fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    format!("{}/{}", base, path)
}
