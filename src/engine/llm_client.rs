use serde::{Deserialize, Serialize};
use reqwest::blocking::Client;
use anyhow::Result;

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
pub fn call_lm_studio(prompt: String) -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::new();

    let req = ChatCompletionRequest {
        model: "local-model".into(),
        temperature: 0.7,
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: prompt,
            }
        ],
    };

    let resp = client
        .post("http://localhost:1234/v1/chat/completions")
        .json(&req)
        .send()?
        .json::<ChatCompletionResponse>()?;

    Ok(resp.choices[0].message.content.clone())
}

pub fn test_connection() -> Result<String> {
    let client = Client::new();

    let resp: serde_json::Value = client
        .get("http://localhost:1234/v1/models")
        .send()?
        .json()?;

    Ok(format!(
        "Connected ({} models available)",
        resp["data"].as_array().map(|a| a.len()).unwrap_or(0)
    ))
}