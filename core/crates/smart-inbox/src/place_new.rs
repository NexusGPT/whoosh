//! Use case: place the mail that has just arrived. Reads the tabs, rules and VIPs, walks the list for each INBOX message,
//! hands the store the batch with its tab (`insert_messages`), then writes the real `decided_by` over the store's
//! provisional 'default' (two commits: the store's NewMessage carries a tab but not who decided it; see the lane notes).
use crate::ask::AskModel;
use crate::error::SmartInboxError;
use crate::facts::{HeaderFacts, MessageFacts};
use crate::load::{apply_headers, complete, load_context, open_reader, write_placements};
use crate::place::{Placement, Placer};
use store::{NewMessage, Store, INBOX_FOLDER};

/// A message as sync hands it over, with what the raw headers and the Gmail IMAP fetch said about it.
pub struct Incoming {
    pub new: NewMessage,
    pub headers: HeaderFacts,
    /// X-GM-LABELS values (Gmail IMAP); empty elsewhere.
    pub gmail_labels: Vec<String>,
}

pub struct PlaceNewMessages<'a> {
    pub store: &'a Store,
    pub model: &'a dyn AskModel,
}

impl PlaceNewMessages<'_> {
    pub fn execute(&self, batch: Vec<Incoming>, now_ms: i64) -> Result<Vec<Placement>, SmartInboxError> {
        let reader = open_reader(self.store.path())?;
        let ctx = load_context(&reader)?;
        let placer = Placer::new(&ctx.tabs, &ctx.rules, self.model);
        let mut placements = Vec::new();
        let mut news = Vec::with_capacity(batch.len());
        for inc in batch {
            let m = &inc.new.message;
            let mut f = MessageFacts {
                message_id: m.id.clone(),
                thread_id: m.thread_id.clone(),
                account_id: m.account_id.clone(),
                from_address: m.from_address.clone(),
                from_name: m.sender_name.clone(),
                unread: !m.read,
                to: m.to.clone(),
                cc: m.cc.clone(),
                subject: m.subject.clone(),
                body: inc.new.body_text.clone().unwrap_or_else(|| m.preview.clone()),
                has_list_header: inc.new.has_list_header,
                gmail_labels: inc.gmail_labels.clone(),
                has_attachment: !m.attachment_names.is_empty(),
                received_at_ms: m.received_at_ms,
                ..Default::default()
            };
            apply_headers(&mut f, &inc.headers);
            complete(&reader, &ctx, &mut f)?;
            let mut new = inc.new;
            new.has_list_header = f.has_list_header;
            if new.message.folder == INBOX_FOLDER {
                let p = placer.place(&f);
                new.tab_id = Some(p.tab_id.clone());
                placements.push(p);
            }
            news.push(new);
        }
        self.store.insert_messages(news)?;
        let to_write = placements.clone();
        self.store.write(move |ctx| write_placements(ctx, &to_write, now_ms).map(|_| ()))?;
        Ok(placements)
    }
}
