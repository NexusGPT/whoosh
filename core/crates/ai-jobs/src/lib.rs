//! Single-shot AI jobs. A job is one thing the model does for the owner: a typed input, a prompt written for this app and an
//! output schema. `Jobs::execute` checks the input, calls the provider through the `CompleteText` port of ai-providers (the key
//! is a `KeychainRef`, read there at the moment of the call), and rebuilds the answer to exactly the schema. An answer that does
//! not fit is refused in words that name the job and the field, never with the model's own text.
//! Tool loops (the chat assistant, the MCP agent) are not here: the agents crate owns them.
mod engine;
pub mod fake;
mod input;
mod jobs;
mod prompt;
pub mod schema;
pub mod spec;
pub mod voice;

pub use engine::{JobOutput, Jobs, RunJob};
pub use input::{Input, MailLine};
pub use spec::{all as all_specs, spec, JobId, JobSpec};
