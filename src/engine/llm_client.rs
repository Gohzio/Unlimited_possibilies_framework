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
    pub use_structured_events: bool,
}

#[derive(Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Serialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    pub json_schema: JsonSchemaWrapper,
}

#[derive(Serialize)]
pub struct JsonSchemaWrapper {
    pub name: String,
    pub strict: bool,
    pub schema: serde_json::Value,
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
                response_format: None,
                max_tokens: None,
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

pub fn call_llm_events_structured(
    narrative: &str,
    raw_events: &str,
    cfg: &LlmConfig,
) -> anyhow::Result<String> {
    if !matches!(cfg.api_mode, LlmApiMode::OpenAiChat) {
        return Err(anyhow!("Structured output is only supported for OpenAI-compatible mode"));
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    let schema_value: serde_json::Value =
        serde_json::from_str(EVENTS_SCHEMA).map_err(|e| anyhow!(e))?;

    let user_payload = format!(
        "NARRATIVE:\n{}\n\nRAW EVENTS (may be invalid):\n{}\n\nReturn ONLY the corrected EVENTS JSON array. Do not invent events.",
        narrative.trim(),
        raw_events.trim()
    );

    let req = ChatCompletionRequest {
        model: cfg.model.clone(),
        temperature: 0.0,
        max_tokens: Some(800),
        response_format: Some(ResponseFormat {
            format_type: "json_schema".to_string(),
            json_schema: JsonSchemaWrapper {
                name: "events_only".to_string(),
                strict: true,
                schema: schema_value,
            },
        }),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: "You normalize the RAW EVENTS into a valid JSON array that matches the schema. Use the narrative only to resolve ambiguity. Never invent new events.".to_string(),
            },
            ChatMessage {
                role: "user".into(),
                content: user_payload,
            },
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

const EVENTS_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "NarrativeEvents",
  "type": "array",
  "items": {
    "oneOf": [
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id", "name", "description"],
        "properties": {
          "type": { "const": "grant_power" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "description"],
        "properties": {
          "type": { "const": "combat" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "speaker", "text"],
        "properties": {
          "type": { "const": "dialogue" },
          "speaker": { "type": "string" },
          "text": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "from", "to"],
        "properties": {
          "type": { "const": "travel" },
          "from": { "type": "string" },
          "to": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "description"],
        "properties": {
          "type": { "const": "rest" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "recipe"],
        "properties": {
          "type": { "const": "craft" },
          "recipe": { "type": "string" },
          "quantity": { "type": "integer", "minimum": 1 },
          "quality": { "type": "string" },
          "result": { "type": "string" },
          "set_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "resource"],
        "properties": {
          "type": { "const": "gather" },
          "resource": { "type": "string" },
          "quantity": { "type": "integer", "minimum": 1 },
          "quality": { "type": "string" },
          "set_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id", "name", "role"],
        "properties": {
          "type": { "const": "add_party_member" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id"],
        "properties": {
          "type": { "const": "party_update" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "details": { "type": "string" },
          "clothing_add": { "type": "array", "items": { "type": "string" } },
          "clothing_remove": { "type": "array", "items": { "type": "string" } },
          "weapons_add": { "type": "array", "items": { "type": "string" } },
          "weapons_remove": { "type": "array", "items": { "type": "string" } },
          "armor_add": { "type": "array", "items": { "type": "string" } },
          "armor_remove": { "type": "array", "items": { "type": "string" } }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "section", "id", "name"],
        "properties": {
          "type": { "const": "section_card_upsert" },
          "section": { "type": "string" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "status": { "type": "string" },
          "details": { "type": "string" },
          "notes": { "type": "string" },
          "tags": { "type": "array", "items": { "type": "string" } },
          "items": { "type": "array", "items": { "type": "string" } }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "section", "id"],
        "properties": {
          "type": { "const": "section_card_remove" },
          "section": { "type": "string" },
          "id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type"],
        "properties": {
          "type": { "const": "player_card_update" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "status": { "type": "string" },
          "details": { "type": "string" },
          "notes": { "type": "string" },
          "tags": { "type": "array", "items": { "type": "string" } },
          "items": { "type": "array", "items": { "type": "string" } }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "minutes"],
        "properties": {
          "type": { "const": "time_passed" },
          "minutes": { "type": "integer", "minimum": 1 },
          "reason": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "name", "role"],
        "properties": {
          "type": { "const": "npc_spawn" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "details": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type"],
        "properties": {
          "type": { "const": "npc_join_party" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "details": { "type": "string" },
          "clothing": { "type": "array", "items": { "type": "string" } },
          "weapons": { "type": "array", "items": { "type": "string" } },
          "armor": { "type": "array", "items": { "type": "string" } }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type"],
        "properties": {
          "type": { "const": "npc_update" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "details": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id"],
        "properties": {
          "type": { "const": "npc_despawn" },
          "id": { "type": "string" },
          "reason": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id"],
        "properties": {
          "type": { "const": "npc_leave_party" },
          "id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "subject_id", "target_id", "delta"],
        "properties": {
          "type": { "const": "relationship_change" },
          "subject_id": { "type": "string" },
          "target_id": { "type": "string" },
          "delta": { "type": "integer" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "stat_id", "delta"],
        "properties": {
          "type": { "const": "modify_stat" },
          "stat_id": { "type": "string" },
          "delta": { "type": "integer" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "amount"],
        "properties": {
          "type": { "const": "add_exp" },
          "amount": { "type": "integer", "minimum": 1 }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "levels"],
        "properties": {
          "type": { "const": "level_up" },
          "levels": { "type": "integer", "minimum": 1 }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "item_id", "slot"],
        "properties": {
          "type": { "const": "equip_item" },
          "item_id": { "type": "string" },
          "slot": { "type": "string" },
          "set_id": { "type": "string" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "item_id"],
        "properties": {
          "type": { "const": "unequip_item" },
          "item_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id", "title", "description"],
        "properties": {
          "type": { "const": "start_quest" },
          "id": { "type": "string" },
          "title": { "type": "string" },
          "description": { "type": "string" },
          "difficulty": { "type": "string" },
          "negotiable": { "type": "boolean" },
          "reward_options": { "type": "array", "items": { "type": "string" } },
          "rewards": { "type": "array", "items": { "type": "string" } },
          "sub_quests": {
            "type": "array",
            "items": {
              "type": "object",
              "additionalProperties": false,
              "required": ["id", "description"],
              "properties": {
                "id": { "type": "string" },
                "description": { "type": "string" },
                "completed": { "type": "boolean" }
              }
            }
          },
          "declinable": { "type": "boolean" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id"],
        "properties": {
          "type": { "const": "update_quest" },
          "id": { "type": "string" },
          "title": { "type": "string" },
          "description": { "type": "string" },
          "status": { "type": "string", "enum": ["active", "completed", "failed"] },
          "difficulty": { "type": "string" },
          "negotiable": { "type": "boolean" },
          "reward_options": { "type": "array", "items": { "type": "string" } },
          "rewards": { "type": "array", "items": { "type": "string" } },
          "sub_quests": {
            "type": "array",
            "items": {
              "type": "object",
              "additionalProperties": false,
              "required": ["id"],
              "properties": {
                "id": { "type": "string" },
                "description": { "type": "string" },
                "completed": { "type": "boolean" }
              }
            }
          }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "topics"],
        "properties": {
          "type": { "const": "request_context" },
          "topics": {
            "oneOf": [
              { "type": "string" },
              { "type": "array", "items": { "type": "string" } }
            ]
          }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "flag"],
        "properties": {
          "type": { "const": "set_flag" },
          "flag": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "reason"],
        "properties": {
          "type": { "const": "request_retcon" },
          "reason": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "item_id", "quantity"],
        "properties": {
          "type": { "const": "add_item" },
          "item_id": { "type": "string" },
          "quantity": { "type": "integer", "minimum": 1 },
          "set_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "item"],
        "properties": {
          "type": { "const": "drop" },
          "item": { "type": "string" },
          "quantity": { "type": "integer" },
          "description": { "type": "string" },
          "set_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "item"],
        "properties": {
          "type": { "const": "spawn_loot" },
          "item": { "type": "string" },
          "quantity": { "type": "integer" },
          "description": { "type": "string" },
          "set_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "currency", "delta"],
        "properties": {
          "type": { "const": "currency_change" },
          "currency": { "type": "string" },
          "delta": { "type": "integer" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id", "name"],
        "properties": {
          "type": { "const": "faction_spawn" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "kind": { "type": "string" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id"],
        "properties": {
          "type": { "const": "faction_update" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "kind": { "type": "string" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id", "delta"],
        "properties": {
          "type": { "const": "faction_rep_change" },
          "id": { "type": "string" },
          "delta": { "type": "integer" }
        }
      }
    ]
  }
}"#;

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
