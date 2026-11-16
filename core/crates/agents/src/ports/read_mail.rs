use contracts::CoreError;

/// One message as the agent may see it: what a citation needs, and the text that may leave the device.
#[derive(Debug, Clone, PartialEq)]
pub struct MailExcerpt {
    pub message_id: String,
    pub thread_id: String,
    pub account_id: String,
    /// the account's display name ("Provanta RFP")
    pub account_name: String,
    pub sender_name: String,
    pub subject: String,
    pub received_at_ms: i64,
    pub text: String,
}

/// The messages with these ids, in the order asked; an unknown id is left out.
pub trait ReadMail: Send + Sync {
    fn execute(&self, message_ids: &[String]) -> Result<Vec<MailExcerpt>, CoreError>;
}
