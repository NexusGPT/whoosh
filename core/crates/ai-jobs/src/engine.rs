//! Run one job: check the input, render the prompt, call the provider, check the answer.
use std::sync::Arc;

use ai_providers::{CompleteText, TextRequest};
use contracts::{ActionFailed, CoreError, KeychainRef};
use serde_json::{Map, Value};

use crate::input::{self, Input};
use crate::schema::parse_answer;
use crate::spec::{spec, JobId, JobSpec};
use crate::prompt;

/// A job's answer, rebuilt to exactly its output schema.
#[derive(Debug, Clone, PartialEq)]
pub struct JobOutput {
    pub job: JobId,
    pub value: Value,
}

pub trait RunJob: Send + Sync {
    fn execute(&self, job: JobId, input: &Input) -> Result<JobOutput, CoreError>;
}

/// Which provider, which model, which key, and the ceiling on an answer. The ceiling is one of the ledger's numbers, passed in.
pub struct Jobs {
    provider: Arc<dyn CompleteText>,
    model: String,
    key: Option<KeychainRef>,
    max_output_tokens: u32,
}

impl Jobs {
    pub fn new(provider: Arc<dyn CompleteText>, model: &str, key: Option<KeychainRef>, max_output_tokens: u32) -> Self {
        Self { provider, model: model.to_string(), key, max_output_tokens }
    }
}

/// The refusal the owner reads: the job's title and the problem in words. The model's own text is never part of it.
fn refused(spec: &JobSpec, problem: &str) -> CoreError {
    CoreError::Action {
        failure: ActionFailed {
            account_name: spec.title.to_string(),
            reason_words: format!("The model's answer could not be used: {problem}. Nothing was changed. Try again, or pick another model in Settings."),
            retryable: true,
            code: None,
        },
    }
}

fn collect(v: &Value, path: &str, out: &mut Vec<String>) {
    match path.split_once('.') {
        Some((head, rest)) => {
            let (name, many) = head.strip_suffix("[]").map_or((head, false), |n| (n, true));
            let child = &v[name];
            if many {
                child.as_array().into_iter().flatten().for_each(|c| collect(c, rest, out));
            } else {
                collect(child, rest, out);
            }
        }
        None => match path.strip_suffix("[]") {
            Some(name) => out.extend(v[name].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from)),
            None => out.extend(v[path].as_str().map(String::from)),
        },
    }
}

fn listed(item: &str, value: &str) -> bool {
    item == value || item.strip_prefix(value).is_some_and(|rest| rest.starts_with(':'))
}

fn check_members(spec: &JobSpec, input: &Map<String, Value>, answer: &Value) -> Result<(), String> {
    for (out_path, in_list) in &spec.members {
        let allowed: Vec<&str> = input.get(*in_list).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
        let mut values = vec![];
        collect(answer, out_path, &mut values);
        if values.iter().any(|v| !allowed.iter().any(|a| listed(a, v))) {
            return Err(format!("\"{out_path}\" names something that is not in \"{in_list}\""));
        }
    }
    Ok(())
}

fn plain(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn check_copied(spec: &JobSpec, input: &Map<String, Value>, answer: &Value) -> Result<(), String> {
    for (out_path, in_field) in &spec.copied {
        let source = match input.get(*in_field) {
            Some(Value::String(s)) => plain(s),
            Some(Value::Array(a)) => a.iter().filter_map(|m| m["text"].as_str().or_else(|| m.as_str())).map(plain).collect::<Vec<_>>().join(""),
            _ => String::new(),
        };
        let mut values = vec![];
        collect(answer, out_path, &mut values);
        if values.iter().any(|v| !source.contains(&plain(v))) {
            return Err(format!("\"{out_path}\" is not written in \"{in_field}\", so it was made up"));
        }
    }
    Ok(())
}

impl RunJob for Jobs {
    fn execute(&self, job: JobId, input: &Input) -> Result<JobOutput, CoreError> {
        let spec = spec(job);
        input::check(spec, input)?;
        let request = TextRequest {
            model: self.model.clone(),
            system: Some(prompt::system(spec)),
            prompt: prompt::user(spec, input),
            max_output_tokens: self.max_output_tokens,
        };
        // A provider's refusal is already in the provider's own words (ai-providers): it passes through unchanged.
        let raw = self.provider.execute(self.key.as_ref(), &request)?;
        let parsed = parse_answer(&raw).map_err(|why| refused(spec, why))?;
        let value = spec.output.check(&parsed, "", input.map()).map_err(|why| refused(spec, &why))?;
        check_members(spec, input.map(), &value).map_err(|why| refused(spec, &why))?;
        check_copied(spec, input.map(), &value).map_err(|why| refused(spec, &why))?;
        Ok(JobOutput { job, value })
    }
}
