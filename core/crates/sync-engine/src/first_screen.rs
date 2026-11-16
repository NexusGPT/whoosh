//! Use case: store an account's first screen (the newest page of its inbox).
use contracts::ActionFailed;
use std::collections::HashMap;

use crate::account::AccountCtx;
use crate::retry;
use crate::state::FolderRow;

/// The inbox folder, else the first folder listed.
pub fn inbox_of(folders: &[String]) -> Option<&String> {
    folders.iter().find(|f| f.as_str() == store::INBOX_FOLDER).or(folders.first())
}

pub async fn first_screen(ctx: &AccountCtx, folders: &[String], stored: &HashMap<String, FolderRow>) -> Result<(), ActionFailed> {
    let Some(inbox) = inbox_of(folders) else { return Ok(()) };
    if stored.get(inbox).is_some_and(|r| r.seen) {
        return Ok(()); // resumed: the first screen is already in the store
    }
    let page = retry::call(ctx, || ctx.source.page(inbox, None, ctx.cfg.first_screen)).await?;
    ctx.persist(inbox, page).await
}
