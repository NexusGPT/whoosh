use crate::domain::event::AgentEvent;

/// Where a run reports, in order. The FFI implements it and forwards `AgentEvent::core_event` to the Agent stream.
pub trait AgentEvents: Send + Sync {
    fn execute(&self, event: AgentEvent);
}
