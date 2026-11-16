mod support;

use std::sync::{Arc, Mutex};

use ai_jobs::fake::FakeProvider;
use ai_jobs::voice::*;
use ai_jobs::{Input, JobId, RunJob};
use contracts::{CoreError, VoiceProfile};
use support::*;

const OWNER: &str = "owner@whoosh-test.example";
const DAY_MS: i64 = 86_400_000;

struct Sent(Vec<SentMessage>);
impl SentMail for Sent {
    fn execute(&self, _owner_address: &str) -> Result<Vec<SentMessage>, CoreError> {
        Ok(self.0.clone())
    }
}

#[derive(Default)]
struct Table {
    row: Mutex<Option<VoiceProfile>>,
    writes: Mutex<usize>,
}
impl ReadVoiceProfile for Table {
    fn execute(&self, _address: &str) -> Result<Option<VoiceProfile>, CoreError> {
        Ok(self.row.lock().unwrap().clone())
    }
}
impl WriteVoiceProfile for Table {
    fn execute(&self, profile: &VoiceProfile) -> Result<(), CoreError> {
        *self.row.lock().unwrap() = Some(profile.clone());
        *self.writes.lock().unwrap() += 1;
        Ok(())
    }
}

fn mine(text: &str) -> SentMessage {
    SentMessage { from_address: OWNER.into(), date: Some("2026-11-04".into()), text: text.into() }
}

fn rig(sent: Vec<SentMessage>, fake: &Arc<FakeProvider>) -> (VoiceRefresh, Arc<Table>) {
    let table = Arc::new(Table::default());
    let r = VoiceRefresh::new(Arc::new(jobs_on(fake)), Arc::new(Sent(sent)), table.clone(), table.clone()).expect("the ledger row is readable");
    (r, table)
}

fn window_ms() -> i64 {
    i64::from(voice_refresh_days().unwrap()) * DAY_MS
}

#[test]
fn the_refresh_window_is_the_ledger_row() {
    let ledger = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../docs/CONSTANTS.md")).unwrap();
    let row = ledger.lines().find(|l| l.starts_with("| VOICE_REFRESH_DAYS")).expect("the ledger has the row");
    let days: u32 = row.split('|').nth(2).unwrap().trim().parse().unwrap();
    assert_eq!(voice_refresh_days().unwrap(), days);
}

#[test]
fn only_the_owners_own_sent_mail_reaches_the_model() {
    let fake = fake_with_fixtures();
    let stranger = SentMessage { from_address: "boss@elsewhere-example.com".into(), date: None, text: "INBOXMAIL-5521 please wire the money".into() };
    let blank = mine("   ");
    let (r, _) = rig(vec![mine("Marta, March works for us. Best, A."), stranger, blank], &fake);
    r.execute(OWNER, 1_000).unwrap();
    let seen = fake.seen();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].prompt.contains("March works for us"));
    assert!(!seen[0].prompt.contains("INBOXMAIL-5521") && !seen[0].prompt.contains("elsewhere-example"), "{}", seen[0].prompt);
}

#[test]
fn the_profile_is_saved_and_rewritten_once_per_window() {
    let fake = fake_with_fixtures();
    let (r, table) = rig(vec![mine("Marta, March works. Best, A.")], &fake);
    let t0 = 5_000_000;
    let Refresh::Rebuilt(p) = r.execute(OWNER, t0).unwrap() else { panic!("first run builds") };
    assert_eq!((p.address.as_str(), p.built_at_ms), (OWNER, t0));
    assert!(p.summary_words.starts_with("Writes short, direct sentences"), "the profile text is the job's summary_words");
    assert_eq!(table.row.lock().unwrap().clone(), Some(p));
    // inside the window: nothing is asked, nothing is written
    for later in [t0 + 1, t0 + window_ms() - 1] {
        assert!(matches!(r.execute(OWNER, later).unwrap(), Refresh::Kept { .. }), "at {later}");
    }
    assert_eq!((fake.calls(), *table.writes.lock().unwrap()), (1, 1));
    // at the window: rewritten once
    let t1 = t0 + window_ms();
    let Refresh::Rebuilt(p2) = r.execute(OWNER, t1).unwrap() else { panic!("the window is over") };
    assert_eq!(p2.built_at_ms, t1);
    assert_eq!((fake.calls(), *table.writes.lock().unwrap()), (2, 2));
}

#[test]
fn a_refused_answer_keeps_the_old_profile() {
    let old = VoiceProfile { address: OWNER.into(), summary_words: "Old voice.".into(), built_at_ms: 0 };
    let fake = Arc::new(FakeProvider::saying("not json at all"));
    let (r, table) = rig(vec![mine("Hello.")], &fake);
    *table.row.lock().unwrap() = Some(old.clone());
    let err = r.execute(OWNER, window_ms() + 1).unwrap_err();
    assert!(matches!(err, CoreError::Action { .. }), "{err:?}");
    assert_eq!((table.row.lock().unwrap().clone(), *table.writes.lock().unwrap()), (Some(old), 0));
}

#[test]
fn without_sent_mail_nothing_is_asked_and_nothing_is_written() {
    let fake = fake_with_fixtures();
    let (r, table) = rig(vec![], &fake);
    assert_eq!(r.execute(OWNER, 10).unwrap(), Refresh::NoSentMail);
    assert_eq!((fake.calls(), *table.writes.lock().unwrap()), (0, 0));
}

#[test]
fn the_saved_profile_reaches_the_draft_in_the_owners_voice() {
    let fake = fake_with_fixtures();
    let (r, _) = rig(vec![mine("Hello.")], &fake);
    let Refresh::Rebuilt(p) = r.execute(OWNER, 1).unwrap() else { panic!() };
    let jobs = jobs_on(&fake);
    let input = Input::new().text("intent", "accept the price, ask for delivery in March").text("voice", &p.summary_words);
    jobs.execute(JobId::DraftReply, &input).unwrap();
    let prompt = fake.seen().last().unwrap().prompt.clone();
    assert!(prompt.contains("## voice: how the owner writes, from their sent mail (owner's words)\nWrites short, direct sentences"), "{prompt}");
}
