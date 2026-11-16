//! The typed input of a job. Built with the methods below or parsed from JSON; checked against the job's declared fields
//! before anything reaches a provider (parse, don't cast).
use contracts::CoreError;
use serde_json::{json, Map, Value};

use crate::spec::{JobSpec, Kind};

/// One line of a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailLine {
    pub from: String,
    pub date: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Input(pub(crate) Map<String, Value>);

impl Input {
    pub fn new() -> Self {
        Self::default()
    }
    /// Input from JSON that crossed a boundary. Its shape is checked against the job when the job runs.
    pub fn from_json(v: Value) -> Result<Self, CoreError> {
        match v {
            Value::Object(m) => Ok(Input(m)),
            _ => Err(CoreError::Invalid { field: "input".into(), reason_words: "the input of a job is an object with one entry per field".into() }),
        }
    }
    pub fn text(mut self, name: &str, value: &str) -> Self {
        self.0.insert(name.into(), Value::String(value.into()));
        self
    }
    pub fn list(mut self, name: &str, items: &[&str]) -> Self {
        self.0.insert(name.into(), json!(items));
        self
    }
    pub fn count(mut self, name: &str, n: u32) -> Self {
        self.0.insert(name.into(), json!(n));
        self
    }
    pub fn messages(mut self, name: &str, lines: &[MailLine]) -> Self {
        let v: Vec<Value> = lines.iter().map(|l| json!({ "from": l.from, "date": l.date, "text": l.text })).collect();
        self.0.insert(name.into(), Value::Array(v));
        self
    }
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }
    pub(crate) fn map(&self) -> &Map<String, Value> {
        &self.0
    }
}

fn bad(field: &str, why: &str) -> CoreError {
    CoreError::Invalid { field: field.into(), reason_words: why.into() }
}

/// Refuse input the job did not declare, a missing required field, and a field of the wrong shape.
pub(crate) fn check(spec: &JobSpec, input: &Input) -> Result<(), CoreError> {
    for name in input.0.keys() {
        if !spec.input.iter().any(|f| f.name == name) {
            return Err(bad(name, &format!("\"{}\" takes no field named {name}", spec.title)));
        }
    }
    for f in &spec.input {
        let v = match input.0.get(f.name) {
            None | Some(Value::Null) if f.required => return Err(bad(f.name, &format!("\"{}\" needs {} ({})", spec.title, f.name, f.what))),
            None | Some(Value::Null) => continue,
            Some(v) => v,
        };
        match f.kind {
            Kind::Owner | Kind::Mail => match v.as_str() {
                Some(s) if s.trim().is_empty() && f.required => return Err(bad(f.name, &format!("{} is empty", f.name))),
                Some(_) => {}
                None => return Err(bad(f.name, &format!("{} must be text", f.name))),
            },
            Kind::List => {
                let items = v.as_array().filter(|a| a.iter().all(Value::is_string));
                match items {
                    Some(a) if a.is_empty() && f.required => return Err(bad(f.name, &format!("{} is an empty list", f.name))),
                    Some(_) => {}
                    None => return Err(bad(f.name, &format!("{} must be a list of text", f.name))),
                }
            }
            Kind::Messages => {
                let ok = v.as_array().is_some_and(|a| a.iter().all(|m| m.get("from").is_some_and(Value::is_string) && m.get("text").is_some_and(Value::is_string)));
                if !ok {
                    return Err(bad(f.name, &format!("{} must be a list of messages, each with a sender and a text", f.name)));
                }
                if v.as_array().is_some_and(Vec::is_empty) && f.required {
                    return Err(bad(f.name, &format!("{} holds no messages", f.name)));
                }
            }
            Kind::Count => {
                if v.as_u64().is_none() {
                    return Err(bad(f.name, &format!("{} must be a whole number", f.name)));
                }
            }
        }
    }
    Ok(())
}
