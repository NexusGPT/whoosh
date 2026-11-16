use contracts::{Command, CommandOutcome, CoreError};

/// Runs an approved proposal's command. Bound to `actions::Actions::run` (Label) and the send path (Send).
pub trait ApplyCommand: Send + Sync {
    fn execute(&self, command: Command) -> Result<CommandOutcome, CoreError>;
}
