//! The output schema of a job: the one description both the prompt and the check of the model's answer are made from.
//! A problem found in an answer is written in words that name the field only, never the model's own text.
use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub enum Schema {
    /// A non-empty string.
    Text { what: &'static str },
    /// Text kept exactly as the model wrote it, spaces included (a continuation starts with a space when the sentence needs one).
    Raw { what: &'static str },
    Flag { what: &'static str },
    /// A number from 0 to 1.
    Score { what: &'static str },
    Pick { what: &'static str, of: &'static [&'static str] },
    /// `max_from` names an input field (a count) that caps the length of the list.
    List { what: &'static str, item: Box<Schema>, max_from: Option<&'static str> },
    Object { fields: Vec<Field> },
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: &'static str,
    pub schema: Schema,
    pub required: bool,
}

pub fn text(what: &'static str) -> Schema {
    Schema::Text { what }
}
pub fn raw(what: &'static str) -> Schema {
    Schema::Raw { what }
}
pub fn flag(what: &'static str) -> Schema {
    Schema::Flag { what }
}
pub fn score(what: &'static str) -> Schema {
    Schema::Score { what }
}
pub fn pick(what: &'static str, of: &'static [&'static str]) -> Schema {
    Schema::Pick { what, of }
}
pub fn list(what: &'static str, item: Schema) -> Schema {
    Schema::List { what, item: Box::new(item), max_from: None }
}
pub fn list_max(what: &'static str, item: Schema, max_from: &'static str) -> Schema {
    Schema::List { what, item: Box::new(item), max_from: Some(max_from) }
}
pub fn req(name: &'static str, schema: Schema) -> Field {
    Field { name, schema, required: true }
}
pub fn opt(name: &'static str, schema: Schema) -> Field {
    Field { name, schema, required: false }
}
pub fn obj(fields: Vec<Field>) -> Schema {
    Schema::Object { fields }
}

impl Schema {
    /// The answer, rebuilt to exactly this schema: unknown fields are dropped, text is trimmed, a pick takes its canonical spelling.
    /// `limits` is the job's input, read only for `max_from`.
    pub fn check(&self, v: &Value, path: &str, limits: &Map<String, Value>) -> Result<Value, String> {
        let at = |p: &str| if p.is_empty() { "the answer".to_string() } else { format!("\"{p}\"") };
        match self {
            Schema::Text { .. } => match v.as_str().map(str::trim) {
                Some(s) if !s.is_empty() => Ok(Value::String(s.to_string())),
                Some(_) => Err(format!("{} is empty", at(path))),
                None => Err(format!("{} is not text", at(path))),
            },
            Schema::Raw { .. } => match v.as_str() {
                Some(s) if !s.trim().is_empty() => Ok(Value::String(s.to_string())),
                Some(_) => Err(format!("{} is empty", at(path))),
                None => Err(format!("{} is not text", at(path))),
            },
            Schema::Flag { .. } => v.as_bool().map(Value::Bool).ok_or_else(|| format!("{} is not true or false", at(path))),
            Schema::Score { .. } => match v.as_f64() {
                Some(n) if (0.0..=1.0).contains(&n) => Ok(v.clone()),
                Some(_) => Err(format!("{} is outside 0 to 1", at(path))),
                None => Err(format!("{} is not a number", at(path))),
            },
            Schema::Pick { of, .. } => {
                let s = v.as_str().map(str::trim).ok_or_else(|| format!("{} is not text", at(path)))?;
                of.iter()
                    .find(|o| o.eq_ignore_ascii_case(s))
                    .map(|o| Value::String((*o).to_string()))
                    .ok_or_else(|| format!("{} is not one of {}", at(path), of.join(", ")))
            }
            Schema::List { item, max_from, .. } => {
                let items = v.as_array().ok_or_else(|| format!("{} is not a list", at(path)))?;
                if let Some(name) = max_from {
                    let cap = limits.get(*name).and_then(Value::as_u64).unwrap_or(0);
                    if items.len() as u64 > cap {
                        return Err(format!("{} has more items than the {cap} asked for", at(path)));
                    }
                }
                let mut out = Vec::with_capacity(items.len());
                for (i, it) in items.iter().enumerate() {
                    out.push(item.check(it, &format!("{path}[{i}]"), limits)?);
                }
                Ok(Value::Array(out))
            }
            Schema::Object { fields } => {
                let map = v.as_object().ok_or_else(|| format!("{} is not an object", at(path)))?;
                let mut out = Map::new();
                for f in fields {
                    let sub = if path.is_empty() { f.name.to_string() } else { format!("{path}.{}", f.name) };
                    match map.get(f.name) {
                        None | Some(Value::Null) if f.required => return Err(format!("\"{sub}\" is missing")),
                        None | Some(Value::Null) => {}
                        Some(x) => {
                            out.insert(f.name.to_string(), f.schema.check(x, &sub, limits)?);
                        }
                    }
                }
                Ok(Value::Object(out))
            }
        }
    }

    /// The field description written into the prompt.
    pub fn describe(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        match self {
            Schema::Text { what } => format!("text ({what})"),
            Schema::Raw { what } => format!("text ({what})"),
            Schema::Flag { what } => format!("true or false ({what})"),
            Schema::Score { what } => format!("a number from 0 to 1 ({what})"),
            Schema::Pick { what, of } => format!("one of {} ({what})", of.iter().map(|o| format!("\"{o}\"")).collect::<Vec<_>>().join(", ")),
            Schema::List { what, item, max_from } => {
                let cap = max_from.map(|m| format!(", at most the number in \"{m}\"")).unwrap_or_default();
                format!("a list ({what}{cap}) of: {}", item.describe(indent))
            }
            Schema::Object { fields } => {
                let mut s = String::from("{\n");
                for f in fields {
                    let o = if f.required { "" } else { " [leave the field out when it does not apply]" };
                    s.push_str(&format!("{pad}  \"{}\": {}{o}\n", f.name, f.schema.describe(indent + 1)));
                }
                s.push_str(&format!("{pad}}}"));
                s
            }
        }
    }

    /// The names of the top-level fields.
    pub fn field_names(&self) -> Vec<&'static str> {
        match self {
            Schema::Object { fields } => fields.iter().map(|f| f.name).collect(),
            _ => vec![],
        }
    }
}

/// The JSON object out of a model's answer: plain, or inside one code fence, or after a line of talk. Anything else is not an answer.
pub fn parse_answer(raw: &str) -> Result<Value, &'static str> {
    let t = raw.trim();
    if t.is_empty() {
        return Err("the answer was empty");
    }
    let unfenced = match t.strip_prefix("```") {
        Some(rest) => {
            let body = rest.split_once('\n').map_or("", |(_, b)| b);
            body.trim_end().strip_suffix("```").unwrap_or(body).trim()
        }
        None => t,
    };
    let try_parse = |s: &str| serde_json::from_str::<Value>(s).ok().filter(Value::is_object);
    if let Some(v) = try_parse(unfenced) {
        return Ok(v);
    }
    if let (Some(a), Some(b)) = (unfenced.find('{'), unfenced.rfind('}')) {
        if a < b {
            if let Some(v) = try_parse(&unfenced[a..=b]) {
                return Ok(v);
            }
        }
    }
    if unfenced.starts_with('{') {
        return Err("the answer was cut off before the JSON object closed");
    }
    Err("the answer was not a JSON object")
}
