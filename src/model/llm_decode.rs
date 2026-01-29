use crate::model::narrative_event::NarrativeEvent;
use serde_json::Value;

/// Decode raw LLM JSON into typed NarrativeEvents
pub fn decode_llm_events(json: &str) -> Result<Vec<NarrativeEvent>, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| format!("Invalid LLM output: {}", e))?;

    let Value::Array(items) = value else {
        return Err("EVENTS must be a JSON array".to_string());
    };

    let mut events = Vec::new();
    for item in items {
        match serde_json::from_value::<NarrativeEvent>(item.clone()) {
            Ok(event) => events.push(event),
            Err(_) => {
                let event_type = item
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                events.push(NarrativeEvent::Unknown {
                    event_type,
                    raw: item,
                });
            }
        }
    }

    Ok(events)
}
