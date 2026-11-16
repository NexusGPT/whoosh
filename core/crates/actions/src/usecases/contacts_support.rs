//! What the three contact use cases share: address words, contact rows, and the per-account mail count.
use crate::engine::new_id;
use store::{StoreError, WriteCtx};

pub(crate) fn norm(address: &str) -> String {
    address.trim().to_lowercase()
}

pub(crate) type ContactRow = store::ContactRecord;

pub(crate) fn contact_of(ctx: &WriteCtx<'_>, address: &str) -> Result<Option<ContactRow>, StoreError> {
    ctx.contact_by_address(address)
}

/// The contact of an address; built from the message headers (name of the newest message, else the address) when there is none.
pub(crate) fn ensure(ctx: &mut WriteCtx<'_>, address: &str, now: i64) -> Result<ContactRow, StoreError> {
    if let Some(c) = contact_of(ctx, address)? {
        return Ok(c);
    }
    let name = ctx.sender_name_of(address)?.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| address.to_string());
    let id = new_id("contact");
    let addresses = vec![address.to_string()];
    ctx.contact_save(&id, &name, &addresses, now)?;
    Ok(ContactRow { id, name, addresses })
}

/// Fold `other` into `keep`: the addresses join, the row goes, the count is recounted.
pub(crate) fn fold(ctx: &mut WriteCtx<'_>, keep: &ContactRow, other: &ContactRow, now: i64) -> Result<ContactRow, StoreError> {
    let mut addresses = keep.addresses.clone();
    for a in &other.addresses {
        if !addresses.iter().any(|x| norm(x) == norm(a)) {
            addresses.push(a.clone());
        }
    }
    ctx.contact_delete(&other.id)?;
    ctx.contact_save(&keep.id, &keep.name, &addresses, now)?;
    Ok(ContactRow { id: keep.id.clone(), name: keep.name.clone(), addresses })
}
