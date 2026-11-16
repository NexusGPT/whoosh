//! An email address split into the parts the Mozilla format's placeholders name.
use contracts::constants::{EMAIL_ADDRESS_MAX, EMAIL_LOCAL_PART_MAX};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailAddress {
    full: String,
    local: String,
    domain: String,
}

impl EmailAddress {
    /// Refuses in words; the reason completes "This address cannot be used: …".
    pub fn parse(input: &str) -> Result<Self, String> {
        let s = input.trim();
        if s.is_empty() {
            return Err("it is empty".into());
        }
        if s.chars().count() as u64 > EMAIL_ADDRESS_MAX {
            return Err(format!("it is longer than {EMAIL_ADDRESS_MAX} characters"));
        }
        let (local, domain) = s.rsplit_once('@').ok_or_else(|| "it has no @".to_string())?;
        if local.is_empty() {
            return Err("there is nothing before the @".into());
        }
        if local.chars().count() as u64 > EMAIL_LOCAL_PART_MAX {
            return Err(format!("the part before the @ is longer than {EMAIL_LOCAL_PART_MAX} characters"));
        }
        if local.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err("the part before the @ contains a space".into());
        }
        let domain = domain.trim_end_matches('.').to_ascii_lowercase();
        let label_ok = |l: &str| !l.is_empty() && !l.starts_with('-') && !l.ends_with('-') && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !domain.contains('.') || !domain.split('.').all(label_ok) {
            return Err(format!("\"{domain}\" is not a mail domain"));
        }
        Ok(Self { full: format!("{local}@{domain}"), local: local.to_string(), domain })
    }

    pub fn full(&self) -> &str { &self.full }
    pub fn local_part(&self) -> &str { &self.local }
    pub fn domain(&self) -> &str { &self.domain }
}
