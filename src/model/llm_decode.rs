use crate::model::narrative_event::NarrativeEvent;
use serde_json::Value;

/// Decode raw LLM JSON into typed NarrativeEvents
pub fn decode_llm_events(json: &str) -> Result<Vec<NarrativeEvent>, String> {
    let normalized = normalize_events_json(json);
    if normalized.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(&normalized)
        .or_else(|e| {
            if let Some(extracted) = extract_json_array(&normalized) {
                serde_json::from_str(&extracted).map_err(|_| format!("Invalid LLM output: {}", e))
            } else {
                Err(format!("Invalid LLM output: {}", e))
            }
        })
        .or_else(|e| {
            if let Some(items) = parse_loose_events(&normalized) {
                Ok(Value::Array(items))
            } else {
                Err(e)
            }
        })
        .or_else(|e| {
            if let Some(items) = parse_single_word_event(&normalized) {
                Ok(Value::Array(items))
            } else {
                Err(e)
            }
        })?;

    let items = match value {
        Value::Array(items) => items,
        Value::Object(mut obj) => {
            if let Some(Value::Array(items)) = obj.remove("events") {
                items
            } else {
                return Err("EVENTS must be a JSON array".to_string());
            }
        }
        _ => return Err("EVENTS must be a JSON array".to_string()),
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

fn normalize_events_json(raw: &str) -> String {
    let mut s = raw.trim().to_string();

    if let Some(pos) = s.find("EVENTS:") {
        s = s[(pos + "EVENTS:".len())..].to_string();
    }

    s = s.trim().to_string();

    if s.starts_with("```") {
        if let Some(first_newline) = s.find('\n') {
            s = s[(first_newline + 1)..].to_string();
        } else {
            return "[]".to_string();
        }
        if let Some(end_fence) = s.rfind("```") {
            s = s[..end_fence].to_string();
        }
        s = s.trim().to_string();
    }

    s
}

fn extract_json_array(s: &str) -> Option<String> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    if end <= start {
        return None;
    }
    Some(s[start..=end].to_string())
}

fn parse_loose_events(s: &str) -> Option<Vec<Value>> {
    let lines: Vec<&str> = s.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let mut items = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let line = match line.strip_prefix("-") {
            Some(rest) => rest.trim(),
            None => continue,
        };
        let (event_type, rest) = line.split_once('{')?;
        let event_type = event_type.trim();
        let mut obj = serde_json::Map::new();
        obj.insert("type".to_string(), Value::String(event_type.to_string()));

        let mut inner = rest.trim().to_string();
        if let Some(end) = inner.rfind('}') {
            inner = inner[..end].to_string();
        }

        for pair in split_pairs(&inner) {
            let (k, v) = match pair.split_once(':') {
                Some(kv) => kv,
                None => continue,
            };
            let key = k.trim().trim_matches('"');
            let val = parse_value(v.trim());
            obj.insert(key.to_string(), val);
        }

        items.push(Value::Object(obj));
    }

    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn parse_single_word_event(s: &str) -> Option<Vec<Value>> {
    let s = s.trim();
    if s.is_empty() || s.contains(char::is_whitespace) {
        return None;
    }
    if s.contains('[') || s.contains('{') || s.contains(':') {
        return None;
    }
    let mut obj = serde_json::Map::new();
    obj.insert("type".to_string(), Value::String(s.to_string()));
    Some(vec![Value::Object(obj)])
}

fn split_pairs(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape = false;
    for ch in s.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        match ch {
            '\\' => {
                current.push(ch);
                escape = true;
            }
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ',' if !in_quotes => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

fn parse_value(raw: &str) -> Value {
    let raw = raw.trim().trim_end_matches(',');
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        return Value::String(raw[1..raw.len() - 1].to_string());
    }
    if let Ok(v) = raw.parse::<i64>() {
        return Value::Number(v.into());
    }
    if let Ok(v) = raw.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(v) {
            return Value::Number(n);
        }
    }
    match raw {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(raw.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::decode_llm_events;
    use crate::model::narrative_event::NarrativeEvent;

    #[test]
    fn decode_valid_json_array() {
        let input = r#"[{\"type\":\"rest\",\"description\":\"Camp\"}]"#;
        let events = decode_llm_events(input).expect("decode");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], NarrativeEvent::Rest { .. }));
    }

    #[test]
    fn decode_loose_events() {
        let input = "- rest { description: \"Camp\" }";
        let events = decode_llm_events(input).expect("decode");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], NarrativeEvent::Rest { .. }));
    }
}
