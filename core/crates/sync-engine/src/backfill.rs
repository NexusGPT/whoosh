//! Use case: backfill one folder's full history, newest to oldest, from its stored anchor.
use contracts::ActionFailed;

use crate::account::AccountCtx;
use crate::retry;
use crate::state::FolderRow;

pub async fn backfill_folder(ctx: &AccountCtx, folder: &str, start: FolderRow) -> Result<(), ActionFailed> {
    if start.done {
        return Ok(());
    }
    let mut before = start.anchor; // None: the folder has no stored page yet, start at its newest
    loop {
        let mut page = retry::call(ctx, || ctx.source.page(folder, before, ctx.cfg.history_page)).await?;
        page.done = page.done || page.oldest.is_none();
        let (done, oldest) = (page.done, page.oldest);
        ctx.persist(folder, page).await?;
        if done {
            return Ok(());
        }
        before = oldest;
    }
}
