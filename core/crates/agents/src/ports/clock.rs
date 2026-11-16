/// Milliseconds since the Unix epoch.
pub trait Clock: Send + Sync {
    fn execute(&self) -> i64;
}
