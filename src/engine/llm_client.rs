use serde::{Deserialize, Serialize};
use reqwest::blocking::Client;
use anyhow::{Result, anyhow};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct LlmConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
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
pub fn call_llm(prompt: String, cfg: &LlmConfig) -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

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

pub fn test_connection(cfg: &LlmConfig) -> Result<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

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

fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    format!("{}/{}", base, path)
}
