//! The accessibility-id grammar: `<surface>.<region>[.<element>...][.<index>]`.
//! surface and region are lowercase words joined by '-'; an element may carry digits and
//! capitals (an account id, a colour hex); a trailing all-digit segment is the index.
use crate::CoreError;

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct AxId {
    pub surface: String,
    pub region: String,
    pub elements: Vec<String>,
    pub index: Option<u32>,
}

fn word(s: &str, lower_only: bool) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars().all(|c| c == '-' || c.is_ascii_digit() || c.is_ascii_lowercase() || (!lower_only && c.is_ascii_uppercase()))
}

#[uniffi::export]
pub fn parse_ax_id(text: String) -> Result<AxId, CoreError> {
    let bad = |why: &str| CoreError::Invalid { field: "ax_id".into(), reason_words: format!("{text:?} {why}") };
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() < 2 {
        return Err(bad("needs at least <surface>.<region>"));
    }
    if !word(parts[0], true) || !word(parts[1], true) {
        return Err(bad("has a surface or region that is not lowercase words joined by '-'"));
    }
    let mut elements: Vec<String> = Vec::new();
    let mut index = None;
    for (i, p) in parts[2..].iter().enumerate() {
        let last = i == parts.len() - 3;
        if last && !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) {
            index = Some(p.parse::<u32>().map_err(|_| bad("has an index too large"))?);
        } else if word(p, false) {
            elements.push((*p).to_string());
        } else {
            return Err(bad("has an empty or malformed element"));
        }
    }
    Ok(AxId { surface: parts[0].into(), region: parts[1].into(), elements, index })
}

#[uniffi::export]
pub fn format_ax_id(id: AxId) -> String {
    let mut s = format!("{}.{}", id.surface, id.region);
    for e in &id.elements {
        s.push('.');
        s.push_str(e);
    }
    if let Some(i) = id.index {
        s.push_str(&format!(".{i}"));
    }
    s
}
