//! The owner said yes to sending mail excerpts to a provider. Recorded once; the question never comes back for it.
//! A no records nothing: nothing was sent, and the next question to that provider asks again.
use contracts::CoreError;

use crate::runtime::AgentPorts;

pub struct GiveConsent {
    ports: AgentPorts,
}

impl GiveConsent {
    pub fn new(ports: AgentPorts) -> GiveConsent {
        GiveConsent { ports }
    }

    pub fn execute(&self, provider_id: &str) -> Result<(), CoreError> {
        self.ports.record_consent.execute(provider_id, self.ports.clock.execute())
    }
}
