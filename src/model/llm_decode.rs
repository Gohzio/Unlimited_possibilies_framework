use crate::model::narrative_event::NarrativeEvent;
use serde_json;

/// Decode raw LLM JSON into typed NarrativeEvents
pub fn decode_llm_events(json: &str) -> Result<Vec<NarrativeEvent>, String> {
    serde_json::from_str::<Vec<NarrativeEvent>>(json)
        .map_err(|e| format!("Invalid LLM output: {}", e))
}
