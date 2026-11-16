//! Reading the model's reply: one JSON object, either a tool call or an answer. Anything else is read as plain answer
//! words with no citations, which the grounding rule then treats like any uncited answer.
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum ModelReply {
    Call { tool: String, args: Value },
    Answer { text: String, cite: Vec<String> },
}

pub fn parse(raw: &str) -> ModelReply {
    let object = raw.find('{').zip(raw.rfind('}')).filter(|(a, b)| a < b).and_then(|(a, b)| serde_json::from_str::<Value>(&raw[a..=b]).ok());
    match object {
        Some(v) if v["tool"].is_string() => ModelReply::Call { tool: v["tool"].as_str().unwrap_or_default().to_string(), args: v },
        Some(v) if v["answer"].is_string() => ModelReply::Answer { text: v["answer"].as_str().unwrap_or_default().to_string(), cite: ids(&v["cite"]) },
        _ => ModelReply::Answer { text: raw.trim().to_string(), cite: Vec::new() },
    }
}

/// A list of ids out of a JSON array; anything that is not a string is skipped.
pub fn ids(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

pub fn text(v: &Value) -> String {
    v.as_str().map(str::trim).unwrap_or_default().to_string()
}
