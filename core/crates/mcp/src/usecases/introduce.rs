//! `initialize`: the app names itself. Whoosh remembers the name, ungranted, so the grant list in Settings can
//! show it and the owner can allow it. A name Whoosh already knows keeps its grant untouched.
use std::sync::Arc;

use contracts::McpGrant;

use crate::domain::McpError;
use crate::ports::{GrantLookup, GrantSave};

pub struct Introduce {
    lookup: Arc<dyn GrantLookup>,
    save: Arc<dyn GrantSave>,
}

impl Introduce {
    pub fn new(lookup: Arc<dyn GrantLookup>, save: Arc<dyn GrantSave>) -> Introduce {
        Introduce { lookup, save }
    }

    pub fn execute(&self, app_name: &str) -> Result<(), McpError> {
        if self.lookup.execute(app_name)?.is_some() {
            return Ok(());
        }
        self.save.execute(&McpGrant { app_name: app_name.to_string(), allowed: false, granted_at_ms: None })
    }
}
