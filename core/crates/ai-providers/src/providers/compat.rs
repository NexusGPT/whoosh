//! The OpenAI-compatible shapes OpenRouter, Ollama and LM Studio share: `GET /models`, `POST /chat/completions`.
use contracts::CoreError;
use serde_json::{json, Value};

use super::Ctx;
use crate::key::Key;
use crate::ports::TextRequest;

pub(crate) fn model_ids(list: &Value) -> Option<Vec<String>> {
    list["data"].as_array().map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
}

pub(crate) fn list(ctx: &Ctx, key: Option<&Key>) -> Result<Vec<String>, CoreError> {
    let answer = ctx.get("/models", key)?;
    model_ids(&answer).ok_or_else(|| ctx.unreadable())
}

pub(crate) fn chat(ctx: &Ctx, key: Option<&Key>, request: &TextRequest) -> Result<String, CoreError> {
    let mut messages = Vec::new();
    if let Some(system) = &request.system {
        messages.push(json!({"role": "system", "content": system}));
    }
    messages.push(json!({"role": "user", "content": request.prompt}));
    let body = json!({"model": request.model, "messages": messages, "max_tokens": request.max_output_tokens});
    let answer = ctx.post("/chat/completions", body, key)?;
    answer["choices"][0]["message"]["content"].as_str().map(String::from).ok_or_else(|| ctx.unreadable())
}
