//! The mail folders of an account as tiles, which messages retrieval may read (this account or all), and the enabled sources of an
//! account in the order the Sources pane shows them.
use contracts::{reply_account_sources, reply_sources_read, ReplyCalendar, ReplySource, ReplySourceRef, ReplySyncProgress, RetrievalScope};
use reply_sources::{enabled_sources, mail_folder_tiles, MailFolderCount, MailScope};

fn mail(account: &str, folder: &str, enabled: bool) -> ReplySource {
    ReplySource { source: ReplySourceRef::MailFolder { account_id: account.into(), folder: folder.into() }, enabled, item_count: 10, progress: ReplySyncProgress::UpToDate }
}

fn count(folder: &str, role: Option<&str>, n: u64) -> MailFolderCount {
    MailFolderCount { folder: folder.into(), role: role.map(str::to_string), count: n }
}

fn rows() -> Vec<ReplySource> {
    vec![
        mail("work", "INBOX", true),
        mail("work", "Sent Mail", true),
        mail("work", "Newsletters", false),
        mail("home", "INBOX", true),
        mail("home", "Factures", true),
        mail("home", "Spam", false),
        ReplySource { source: ReplySourceRef::LocalFolder { path: "~/Documents/Contrats".into() }, enabled: true, item_count: 214, progress: ReplySyncProgress::UpToDate },
    ]
}

#[test]
fn this_account_reads_only_its_own_enabled_folders() {
    let s = MailScope::new("work", RetrievalScope::ThisAccount, &rows());
    assert!(s.admits("work", "INBOX") && s.admits("work", "Sent Mail"));
    assert!(!s.admits("work", "Newsletters"), "a folder switched off is out");
    assert!(!s.admits("home", "INBOX"), "another account is out with Ce compte");
    assert!(!s.admits("work", "inbox"), "a folder is named exactly");
    assert_eq!(s.folders(), &[("work".to_string(), "INBOX".to_string()), ("work".to_string(), "Sent Mail".to_string())]);
}

#[test]
fn all_accounts_reads_every_enabled_folder_this_account_first() {
    let s = MailScope::new("home", RetrievalScope::AllAccounts, &rows());
    assert!(s.admits("work", "INBOX") && s.admits("home", "Factures"));
    assert!(!s.admits("home", "Spam") && !s.admits("work", "Newsletters"), "switched off stays out with Tous les comptes");
    let accounts: Vec<&str> = s.folders().iter().map(|(a, _)| a.as_str()).collect();
    assert_eq!(accounts, vec!["home", "home", "work", "work"]);
}

#[test]
fn an_account_card_lists_its_folders_inbox_then_sent_then_by_name() {
    let folders = vec![count("Newsletters", None, 6310), count("clients / Oranta", None, 1840), count("Envoyés", Some("sent"), 4812), count("Clients", None, 2960), count("INBOX", None, 9204), count("Clients / Lorvan Hotels", None, 1120)];
    let mut stored = vec![mail("work", "INBOX", true), mail("work", "Envoyés", true), mail("work", "Clients", true), mail("work", "Gone", true), mail("other", "Clients", true), mail("work", "Newsletters", false)];
    stored[2].progress = ReplySyncProgress::Syncing { percent: 62 };
    let card = mail_folder_tiles("work", folders, &stored);
    let names: Vec<String> = card.folders.iter().map(|s| match &s.source { ReplySourceRef::MailFolder { folder, .. } => folder.clone(), _ => unreachable!() }).collect();
    assert_eq!(names, vec!["INBOX", "Envoyés", "Clients", "Clients / Lorvan Hotels", "clients / Oranta", "Newsletters"]);
    assert_eq!(card.folders[0].item_count, 9204, "the count is the store's, not the row's");
    assert_eq!(card.folders[2].progress, ReplySyncProgress::Syncing { percent: 62 });
    assert!(!card.folders[3].enabled && card.folders[3].progress == ReplySyncProgress::NotStarted, "a folder with no row is off and not started");
    assert!(!card.folders[5].enabled, "a folder whose row is switched off is off");
    assert_eq!((card.enabled_folders, card.total_folders, card.enabled_items), (3, 6, 9204 + 4812 + 2960), "a gone folder is not a tile; another account's row is not this one's");
}

#[test]
fn enabled_sources_follow_the_pane_order_and_the_scope() {
    let work = reply_account_sources("work".into(), rows());
    let home = reply_account_sources("home".into(), rows());
    let locals = vec![rows()[6].clone(), ReplySource { source: ReplySourceRef::LocalFolder { path: "~/Off".into() }, enabled: false, item_count: 1, progress: ReplySyncProgress::UpToDate }];
    let cals = vec![
        ReplyCalendar { calendar_id: "cal-1".into(), name: "Travail".into(), account_label: "work".into(), colour: "#007AFF".into(), enabled: true },
        ReplyCalendar { calendar_id: "cal-2".into(), name: "Personnel".into(), account_label: "home".into(), colour: "#34C759".into(), enabled: false },
    ];
    let this = reply_sources_read(vec![work.clone(), home.clone()], locals.clone(), cals.clone(), RetrievalScope::ThisAccount);
    let mf = |a: &str, f: &str| ReplySourceRef::MailFolder { account_id: a.into(), folder: f.into() };
    let local = ReplySourceRef::LocalFolder { path: "~/Documents/Contrats".into() };
    let cal = ReplySourceRef::Calendar { calendar_id: "cal-1".into() };
    assert_eq!(enabled_sources(&this, "home"), vec![mf("home", "INBOX"), mf("home", "Factures"), local.clone(), cal.clone()]);
    let all = reply_sources_read(vec![work, home], locals, cals, RetrievalScope::AllAccounts);
    assert_eq!(enabled_sources(&all, "home"), vec![mf("home", "INBOX"), mf("home", "Factures"), mf("work", "INBOX"), mf("work", "Sent Mail"), local, cal]);
}
