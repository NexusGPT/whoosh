//! Merge two addresses into one contact: one contact id carrying both, with the per-account mail count equal to the sum.
use super::contacts_support::{ensure, fold, norm};
use crate::engine::Env;
use contracts::CoreError;

pub struct MergeContact {
    env: Env,
}

impl MergeContact {
    pub fn new(env: Env) -> MergeContact {
        MergeContact { env }
    }

    /// The contact of `keep_address` takes the addresses of the contact of `other_address`. Returns the surviving contact id.
    pub fn execute(&self, keep_address: String, other_address: String) -> Result<String, CoreError> {
        let (keep_address, other_address) = (norm(&keep_address), norm(&other_address));
        if keep_address.is_empty() || other_address.is_empty() || !keep_address.contains('@') || !other_address.contains('@') {
            return Err(CoreError::Invalid { field: "address".into(), reason_words: "both addresses need an @".into() });
        }
        let now = self.env.now();
        Ok(self.env.store.write(move |ctx| {
            let keep = ensure(ctx, &keep_address, now)?;
            let other = ensure(ctx, &other_address, now)?;
            if keep.id == other.id {
                return Ok(keep.id);
            }
            Ok(fold(ctx, &keep, &other, now)?.id)
        })?)
    }
}
