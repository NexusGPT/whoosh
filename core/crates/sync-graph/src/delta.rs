//! One delta round for one folder. Graph's delta query is per folder: the first round (no cursor)
//! walks the whole folder, every later round starts from the stored `@odata.deltaLink` and gets only
//! what changed since. The cursor stored is that link, as Graph hands it out.
use std::collections::HashSet;

use contracts::{CoreError, SyncCursor};
use reqwest::Method;

use crate::{
    client::GraphMailbox,
    model::{invalid, parse, DeltaChange, DeltaPage},
};

#[derive(Debug, Clone, PartialEq)]
pub struct DeltaRound {
    pub folder_id: String,
    pub changes: Vec<DeltaChange>,
    /// Store this; pass it to the next round.
    pub cursor: SyncCursor,
    pub pages: u32,
    /// Microsoft said the stored cursor is too old (410), so this round walked the whole folder:
    /// the engine reconciles the folder instead of only applying changes.
    pub reset: bool,
}

struct Walk {
    changes: Vec<DeltaChange>,
    delta_link: String,
    pages: u32,
}

const SELECT: &str = "id,conversationId,subject,bodyPreview,receivedDateTime,isRead,from,toRecipients,ccRecipients,bccRecipients,unsubscribeData";

impl GraphMailbox {
    fn initial_delta_url(&self, folder_id: &str) -> Result<String, CoreError> {
        let mut u = self.mailbox_url(&["mailFolders", folder_id, "messages", "delta"])?;
        u.query_pairs_mut().append_pair("$select", SELECT);
        Ok(u.to_string())
    }

    async fn walk(&self, start: String, folder_name: &str) -> Result<Option<Walk>, CoreError> {
        let headers = self.page_header();
        let mut url = start;
        let mut seen = HashSet::new();
        let mut changes = Vec::new();
        let mut pages = 0u32;
        loop {
            if !seen.insert(url.clone()) {
                return Err(invalid("@odata.nextLink", "repeats a link already followed"));
            }
            self.check_origin(&url)?;
            let reply = self.send(Method::GET, &url, &headers, None, true).await?;
            if reply.status == 410 {
                return Ok(None);
            }
            let page: DeltaPage = parse(&reply.body, "delta page")?;
            pages += 1;
            for item in page.value {
                changes.push(item.into_change(&self.cfg.account_id, folder_name)?);
            }
            match (page.next_link, page.delta_link) {
                (Some(next), _) => url = next,
                (None, Some(delta)) => {
                    self.check_origin(&delta)?;
                    return Ok(Some(Walk { changes, delta_link: delta, pages }));
                }
                (None, None) => return Err(invalid("@odata.deltaLink", "the last delta page has neither a next link nor a delta link")),
            }
        }
    }

    pub async fn delta_round(&self, folder_id: &str, folder_name: &str, cursor: Option<&SyncCursor>) -> Result<DeltaRound, CoreError> {
        let (first, had_cursor) = match cursor {
            Some(SyncCursor::GraphDelta { token }) => (token.clone(), true),
            Some(_) => return Err(invalid("cursor", "a Graph folder needs a Graph delta cursor")),
            None => (self.initial_delta_url(folder_id)?, false),
        };
        let (walk, reset) = match self.walk(first, folder_name).await? {
            Some(w) => (w, false),
            None if had_cursor => match self.walk(self.initial_delta_url(folder_id)?, folder_name).await? {
                Some(w) => (w, true),
                None => return Err(invalid("delta", "Microsoft answered 410 to a fresh delta query")),
            },
            None => return Err(invalid("delta", "Microsoft answered 410 to a fresh delta query")),
        };
        Ok(DeltaRound { folder_id: folder_id.to_string(), changes: walk.changes, cursor: SyncCursor::GraphDelta { token: walk.delta_link }, pages: walk.pages, reset })
    }
}
