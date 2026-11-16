//! Which mail retrieval may read when it answers from one account: the enabled folders of that account ("Ce compte"), or the enabled
//! folders of every account ("Tous les comptes"). Built once per reply; `admits` is a set lookup, cheap enough for every hit.
use contracts::{ReplySource, ReplySourceRef, RetrievalScope};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailScope {
    folders: Vec<(String, String)>,
    set: HashSet<(String, String)>,
}

impl MailScope {
    /// `sources` = every reply-source row (all kinds, all accounts); only enabled mail folders count.
    pub fn new(account_id: &str, scope: RetrievalScope, sources: &[ReplySource]) -> MailScope {
        let mut mine = Vec::new();
        let mut others = Vec::new();
        for s in sources.iter().filter(|s| s.enabled) {
            if let ReplySourceRef::MailFolder { account_id: a, folder } = &s.source {
                if a == account_id {
                    mine.push((a.clone(), folder.clone()));
                } else if scope == RetrievalScope::AllAccounts {
                    others.push((a.clone(), folder.clone()));
                }
            }
        }
        mine.extend(others);
        let set = mine.iter().cloned().collect();
        MailScope { folders: mine, set }
    }

    /// True when a message of `account_id` in `folder` may be read for this reply.
    pub fn admits(&self, account_id: &str, folder: &str) -> bool {
        self.set.contains(&(account_id.to_string(), folder.to_string()))
    }

    /// The (account, folder) pairs in scope: the replying account's first, in the order of `sources`.
    pub fn folders(&self) -> &[(String, String)] {
        &self.folders
    }
}
