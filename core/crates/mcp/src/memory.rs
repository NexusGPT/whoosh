//! A grant store that lives in memory: the default for tests and for a host that has not bound the SQLite
//! table yet. First-seen order is kept, as the grant list shows it.
use std::sync::Mutex;

use contracts::McpGrant;

use crate::domain::McpError;
use crate::ports::{GrantList, GrantLookup, GrantSave};

#[derive(Default)]
pub struct MemoryGrants {
    rows: Mutex<Vec<McpGrant>>,
}

impl MemoryGrants {
    pub fn new() -> MemoryGrants {
        MemoryGrants::default()
    }

    fn rows(&self) -> std::sync::MutexGuard<'_, Vec<McpGrant>> {
        self.rows.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl GrantLookup for MemoryGrants {
    fn execute(&self, app_name: &str) -> Result<Option<McpGrant>, McpError> {
        Ok(self.rows().iter().find(|g| g.app_name == app_name).cloned())
    }
}

impl GrantSave for MemoryGrants {
    fn execute(&self, grant: &McpGrant) -> Result<(), McpError> {
        let mut rows = self.rows();
        match rows.iter_mut().find(|g| g.app_name == grant.app_name) {
            Some(row) => *row = grant.clone(),
            None => rows.push(grant.clone()),
        }
        Ok(())
    }
}

impl GrantList for MemoryGrants {
    fn execute(&self) -> Result<Vec<McpGrant>, McpError> {
        Ok(self.rows().clone())
    }
}
