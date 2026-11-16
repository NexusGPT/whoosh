//! A JMAP email -> the contract's `Message`. Pure: no I/O, so a test can hold it to the words.
use std::collections::HashMap;

use contracts::Message;
use jmap_client::email::{Email, Property};
use jmap_client::Get;

use crate::port::{FetchOptions, FetchedMessage, Folder};

/// The properties every fetch asks for, once, so first screen, history and resync agree.
pub(crate) fn properties() -> Vec<Property> {
    vec![
        Property::Id,
        Property::ThreadId,
        Property::MailboxIds,
        Property::Keywords,
        Property::ReceivedAt,
        Property::From,
        Property::To,
        // contracts delta 12
        Property::Cc,
        Property::Bcc,
        Property::Subject,
        Property::Preview,
        Property::Attachments,
        // contracts-d11: the List-Unsubscribe headers, raw (RFC 8621 header:<name>:asText; jmap-client 0.4.3 cannot read its own asRaw form back)
        Property::Header(jmap_client::email::Header::as_text("List-Unsubscribe", false)),
        Property::Header(jmap_client::email::Header::as_text("List-Unsubscribe-Post", false)),
    ]
}

fn role_of<'a>(folders: &'a HashMap<String, Folder>, id: &str) -> Option<&'a str> {
    folders.get(id).and_then(|f| f.role.as_deref())
}

pub(crate) fn to_fetched(
    account_id: &str,
    folders: &HashMap<String, Folder>,
    mut email: Email<Get>,
    opts: FetchOptions,
) -> Option<FetchedMessage> {
    let server_id = email.id()?.to_string();
    let mut mailbox_ids: Vec<String> = email.mailbox_ids().into_iter().map(str::to_string).collect();
    mailbox_ids.sort();
    let in_inbox = mailbox_ids.iter().any(|m| role_of(folders, m) == Some("inbox"));
    let in_archive = mailbox_ids.iter().any(|m| role_of(folders, m) == Some("archive"));
    let folder_id = mailbox_ids.iter().find(|m| role_of(folders, m) == Some("inbox")).or_else(|| mailbox_ids.first());
    let folder = folder_id.and_then(|m| folders.get(m)).map(|f| f.name.clone()).unwrap_or_default();

    let keywords: Vec<String> = email.keywords().into_iter().map(str::to_string).collect();
    let from = email.take_from().and_then(|v| v.into_iter().next());
    let (sender_name, from_address) = match from {
        Some(a) => {
            let address = a.email().to_string();
            let name = a.name().filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| address.clone());
            (name, address)
        }
        None => (String::new(), String::new()),
    };
    let to = email.take_to().map(|v| v.into_iter().map(|a| a.email().to_string()).collect()).unwrap_or_default();
    // contracts delta 12: Cc and Bcc (the server returns Bcc only where it kept it: the owner's own sent copies)
    let cc = email.take_cc().map(|v| v.into_iter().map(|a| a.email().to_string()).collect()).unwrap_or_default();
    let bcc = email.take_bcc().map(|v| v.into_iter().map(|a| a.email().to_string()).collect()).unwrap_or_default();
    let preview: String = email.take_preview().unwrap_or_default().chars().take(opts.preview_chars).collect();
    let attachment_names = email
        .attachments()
        .map(|parts| parts.iter().filter_map(|p| p.name().map(str::to_string)).collect())
        .unwrap_or_default();
    let thread_id = email.take_thread_id().unwrap_or_else(|| server_id.clone());

    let raw = |name: &str| match email.header(&jmap_client::email::Header::as_text(name, false)) {
        Some(jmap_client::email::HeaderValue::AsText(t)) => Some(t.clone()),
        _ => None,
    };
    let (list_unsubscribe, list_unsubscribe_one_click) =
        contracts::list_unsubscribe_from_headers(raw("List-Unsubscribe").as_deref(), raw("List-Unsubscribe-Post").as_deref());
    let message = Message { list_unsubscribe, list_unsubscribe_one_click, cc, bcc, is_outgoing: false,
        id: format!("{account_id}:{server_id}"),
        thread_id: format!("{account_id}:{thread_id}"),
        account_id: account_id.to_string(),
        sender_name,
        from_address,
        to,
        subject: email.take_subject().unwrap_or_default(),
        preview,
        received_at_ms: email.received_at().unwrap_or(0).saturating_mul(1000),
        read: keywords.iter().any(|k| k == "$seen"),
        archived: in_archive && !in_inbox,
        folder,
        attachment_names,
        login_code: None,
        trackers_blocked: 0,
        reply_to_name: None,
    };
    Some(FetchedMessage { server_id, mailbox_ids, keywords, message })
}

#[cfg(test)]
mod d11_tests {
    use super::*;

    #[test]
    fn the_list_unsubscribe_header_reaches_the_message() {
        let email: Email<Get> = serde_json::from_value(serde_json::json!({
            "id": "e1", "threadId": "t1", "mailboxIds": {}, "keywords": {}, "subject": "s",
            "header:List-Unsubscribe:asText": " <https://l.example/u/1>, <mailto:leave@l.example>",
            "header:List-Unsubscribe-Post:asText": " List-Unsubscribe=One-Click"
        })).unwrap();
        let f = to_fetched("acc", &HashMap::new(), email, FetchOptions { preview_chars: 10 }).expect("mapped");
        assert_eq!(f.message.list_unsubscribe, vec!["<https://l.example/u/1>".to_string(), "<mailto:leave@l.example>".into()]);
        assert!(f.message.list_unsubscribe_one_click);
        assert!(properties().iter().any(|p| p.to_string() == "header:List-Unsubscribe:asText"), "the fetch asks for it");
    }

    #[test]
    fn cc_and_bcc_reach_the_message() {
        let email: Email<Get> = serde_json::from_value(serde_json::json!({
            "id": "e2", "threadId": "t2", "mailboxIds": {}, "keywords": {}, "subject": "s",
            "cc": [{"name": "Tom", "email": "tom@y.example"}, {"email": "sara@z.example"}],
            "bcc": [{"email": "boss@x.example"}]
        })).unwrap();
        let f = to_fetched("acc", &HashMap::new(), email, FetchOptions { preview_chars: 10 }).expect("mapped");
        assert_eq!(f.message.cc, vec!["tom@y.example".to_string(), "sara@z.example".into()]);
        assert_eq!(f.message.bcc, vec!["boss@x.example".to_string()]);
        assert!(properties().iter().any(|p| p.to_string() == "cc") && properties().iter().any(|p| p.to_string() == "bcc"), "the fetch asks for them");
    }
}
