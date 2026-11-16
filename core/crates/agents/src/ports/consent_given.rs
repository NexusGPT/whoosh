/// Has the owner said yes to sending mail excerpts to this provider?
pub trait ConsentGiven: Send + Sync {
    fn execute(&self, provider_id: &str) -> bool;
}
