//! Contact rows built from the message headers and, after the owner's permission, the device's Contacts.
use super::contacts_support::{contact_of, fold, norm, ContactRow};
use crate::engine::{new_id, Env};
use crate::port::ContactsDirectory;
use contracts::CoreError;
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct BuildContacts {
    env: Env,
}

impl BuildContacts {
    pub fn new(env: Env) -> BuildContacts {
        BuildContacts { env }
    }

    /// One contact per sender address that has none yet (never one for the owner's own addresses); every contact's
    /// per-account count is recounted. Returns how many contacts were created.
    pub fn from_headers(&self) -> Result<usize, CoreError> {
        let now = self.env.now();
        Ok(self.env.store.write(move |ctx| {
            let mut created = 0;
            for (address, name) in ctx.outside_senders()? {
                if contact_of(ctx, &address)?.is_none() {
                    let name = if name.trim().is_empty() { address.clone() } else { name };
                    ctx.contact_save(&new_id("contact"), &name, std::slice::from_ref(&address), now)?;
                    created += 1;
                }
            }
            for c in ctx.contacts_all()? {
                ctx.contact_save(&c.id, &c.name, &c.addresses, now)?;
            }
            Ok(created)
        })?)
    }

    /// Fold the device's Contacts in: a person who shares an address with an existing contact joins it (and the device name
    /// wins); others become new contacts. `None` = the owner has not given permission, nothing was read or written.
    pub fn from_device(&self, directory: Arc<dyn ContactsDirectory>) -> Result<Option<usize>, CoreError> {
        let Ok(people) = directory.execute() else { return Ok(None) };
        let now = self.env.now();
        Ok(Some(self.env.store.write(move |ctx| {
            let mut touched = 0;
            for person in people {
                let addresses: Vec<String> = person.addresses.iter().map(|a| norm(a)).filter(|a| a.contains('@')).collect();
                if addresses.is_empty() {
                    continue;
                }
                let mut seen: BTreeMap<String, ContactRow> = BTreeMap::new();
                for a in &addresses {
                    if let Some(c) = contact_of(ctx, a)? {
                        seen.entry(c.id.clone()).or_insert(c);
                    }
                }
                let mut existing = seen.into_values();
                let mut keep = match existing.next() {
                    Some(c) => c,
                    None => ContactRow { id: new_id("contact"), name: person.name.clone(), addresses: Vec::new() },
                };
                keep.name = person.name.clone();
                for other in existing {
                    keep = fold(ctx, &keep, &other, now)?;
                }
                for a in &addresses {
                    if !keep.addresses.iter().any(|x| norm(x) == *a) {
                        keep.addresses.push(a.clone());
                    }
                }
                ctx.contact_save(&keep.id, &keep.name, &keep.addresses, now)?;
                touched += 1;
            }
            Ok(touched)
        })?))
    }
}
