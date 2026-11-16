//! The agents runtime against a scripted provider and the mac-main fixture rows. Nothing leaves the process.
mod support;

use agents::builtin::{writer, WRITER_INSTRUCTIONS};
use agents::domain::answer::NOTHING_FOUND;
use agents::{allowed_words, AgentEvent, AnswerKind, AskOutcome};
use contracts::constants::{AGENT_CONTEXT_CHUNKS, AGENT_MAX_STEPS};
use contracts::AgentPermission::{Draft, Label, ReadMail, Search, SendWithConfirmation};
use contracts::{CoreEvent, ProposalDecision};
use support::{Harness, MAX_TOKENS};

const QUESTION: &str = "what did suppliers quote me this spring?";
const SEARCH: &str = r#"{"tool":"search","query":"quote procurement"}"#;
const PROPOSE: &str = r#"{"tool":"propose_label","message_ids":["msg-01","msg-10"],"label":"Supplier quotes"}"#;
const ANSWER: &str = r#"{"answer":"Linea Supply quoted the Q1 print run.","cite":["msg-01"]}"#;

#[test]
fn an_agent_without_label_is_never_offered_the_label_tool() {
    let h = Harness::new(&[SEARCH, PROPOSE, ANSWER]);
    h.runtime.give_consent("cloud").unwrap();
    let turn = h.finished(h.agent(&[ReadMail, Search]), QUESTION);

    let seen = h.provider.seen();
    assert_eq!(seen.len(), 3, "search, the refused label call, the answer");
    for call in &seen {
        assert!(!call.system.contains("propose_label"), "label tool offered: {}", call.system);
        assert!(!call.system.contains("label"), "label named to the model: {}", call.system);
        assert!(call.system.contains("You may do only these things: read mail, search."), "{}", call.system);
    }
    assert!(seen[2].prompt.contains("propose_label: this agent may not do that"), "the refusal reaches the model: {}", seen[2].prompt);
    assert!(turn.proposals.is_empty());
    assert_eq!(h.events.count(|e| matches!(e, AgentEvent::ProposalReady { .. })), 0);
    assert!(h.labels.assignments().is_empty());
    assert_eq!(h.labels.applied(), 0);

    // The same script on an agent that may label is offered the tool: the check above can tell the two apart.
    let control = Harness::new(&[SEARCH, PROPOSE, ANSWER]);
    control.runtime.give_consent("cloud").unwrap();
    let turn = control.finished(control.agent(&[ReadMail, Search, Label]), QUESTION);
    assert!(control.provider.seen()[0].system.contains("propose_label"));
    assert_eq!(turn.proposals.len(), 1);
}

#[test]
fn a_proposal_changes_nothing_until_approve_and_applies_exactly_once() {
    let h = Harness::new(&[SEARCH, PROPOSE, ANSWER]);
    h.runtime.give_consent("cloud").unwrap();
    let turn = h.finished(h.agent(&[ReadMail, Search, Label]), "label these as Supplier quotes");

    let proposal = turn.proposals.first().expect("one proposal").clone();
    assert_eq!(proposal.message_ids, vec!["msg-01".to_string(), "msg-10".to_string()]);
    assert_eq!(proposal.label.as_deref(), Some("Supplier quotes"));
    assert_eq!(proposal.action_words, "Label these 2 messages as \u{201c}Supplier quotes\u{201d}");
    assert_eq!(proposal.decision, ProposalDecision::Pending);
    assert!(h.labels.assignments().is_empty(), "label_assignment changed before approve");
    assert_eq!(h.labels.applied(), 0);
    assert_eq!(h.events.count(|e| matches!(e, AgentEvent::ProposalReady { proposal: p } if p.id == proposal.id)), 1);

    h.runtime.decide_proposal(&proposal.id, true).expect("approve applies");
    let want = [("Supplier quotes".to_string(), "msg-01".to_string()), ("Supplier quotes".to_string(), "msg-10".to_string())];
    assert_eq!(h.labels.assignments(), want.into_iter().collect());
    assert_eq!(h.labels.applied(), 1);
    assert!(matches!(h.proposals.get(&proposal.id).unwrap().proposal.decision, ProposalDecision::Approved { .. }));

    assert!(h.runtime.decide_proposal(&proposal.id, true).is_err(), "a second approve is refused");
    assert!(h.runtime.decide_proposal(&proposal.id, false).is_err(), "an approved proposal cannot be dismissed");
    assert_eq!(h.labels.applied(), 1, "applied exactly once");
}

#[test]
fn a_dismissed_proposal_never_applies_and_a_refused_one_waits_again() {
    let h = Harness::new(&[SEARCH, PROPOSE, ANSWER]);
    h.runtime.give_consent("cloud").unwrap();
    let agent = h.agent(&[ReadMail, Search, Label]);

    let first = h.finished(agent.clone(), "label these").proposals[0].clone();
    h.runtime.decide_proposal(&first.id, false).unwrap();
    assert!(h.runtime.decide_proposal(&first.id, true).is_err(), "a dismissed proposal cannot be approved later");
    assert_eq!(h.labels.applied(), 0);
    assert!(h.labels.assignments().is_empty());

    h.provider.script(&[SEARCH, PROPOSE, ANSWER]);
    let second = h.finished(agent, "label these").proposals[0].clone();
    assert_ne!(first.id, second.id);
    *h.labels.refuse_next.lock().unwrap() = true;
    let refused = h.runtime.decide_proposal(&second.id, true).unwrap_err();
    assert_eq!(refused.to_string(), "Studio: the server refused the label");
    assert_eq!(h.proposals.get(&second.id).unwrap().proposal.decision, ProposalDecision::Pending);
    assert!(h.labels.assignments().is_empty());
    h.runtime.decide_proposal(&second.id, true).unwrap();
    assert_eq!(h.labels.applied(), 1);
}

#[test]
fn every_answer_cites_only_messages_it_was_shown_that_exist() {
    let cases: [(&str, Vec<&str>); 4] = [
        (r#"{"answer":"Linea Supply quoted 12,000 units.","cite":["msg-01","msg-99","msg-05","msg-01"]}"#, vec!["msg-01"]),
        (r#"{"answer":"Two quotes came in.","cite":["msg-10","msg-01"]}"#, vec!["msg-10", "msg-01"]),
        (r#"{"answer":"Suppliers quoted a lot.","cite":["msg-77"]}"#, vec![]),
        ("Suppliers quoted a lot.", vec![]),
    ];
    for (reply, want) in cases {
        let h = Harness::new(&[SEARCH, reply]);
        h.runtime.give_consent("cloud").unwrap();
        let turn = h.finished(h.agent(&[ReadMail, Search]), QUESTION);
        let ids: Vec<&str> = turn.answer.citations.iter().map(|c| c.message_id.as_str()).collect();
        assert_eq!(ids, want, "reply {reply}");
        let fixture = h.mail.ids();
        let shown: Vec<String> = h.mail.read.lock().unwrap().clone();
        for c in &turn.answer.citations {
            assert!(fixture.contains(&c.message_id) && shown.contains(&c.message_id), "{} does not resolve", c.message_id);
        }
        if want.is_empty() {
            assert_eq!(turn.answer.text, NOTHING_FOUND, "an uncited answer is not passed off as one: {reply}");
        } else {
            assert_ne!(turn.answer.text, NOTHING_FOUND);
        }
        let chunks: String = h.events.all().iter().filter_map(|e| if let AgentEvent::Chunk { text } = e { Some(text.clone()) } else { None }).collect();
        assert_eq!(chunks, turn.answer.text, "the chunks are the answer");
    }

    let h = Harness::new(&[SEARCH, ANSWER]);
    h.runtime.give_consent("cloud").unwrap();
    let c = h.finished(h.agent(&[ReadMail, Search]), QUESTION).answer.citations.remove(0);
    let row = support::fixture_rows().into_iter().find(|r| r.message_id == "msg-01").unwrap();
    assert_eq!((c.account_id.as_str(), c.sender_name.as_str(), c.received_at_ms), ("acc-04", "Linea Supply", row.received_at_ms));
    assert_eq!(c.account_name, row.account_name);
    assert!(!c.account_name.is_empty(), "the chip shows the account's display name");
    assert_eq!(c.citation().message_id, "msg-01");
}

#[test]
fn consent_is_asked_once_per_provider_before_any_mail_leaves() {
    let h = Harness::new(&[SEARCH, ANSWER]);
    let agent = h.agent(&[ReadMail, Search]);
    let consent_asks = || h.events.count(|e| matches!(e, AgentEvent::ConsentNeeded { .. }));

    for _ in 0..2 {
        match h.ask(agent.clone(), QUESTION) {
            AskOutcome::NeedsConsent { provider_id, provider_name } => assert_eq!((provider_id.as_str(), provider_name.as_str()), ("cloud", "OpenAI")),
            other => panic!("sent without consent: {other:?}"),
        }
    }
    assert_eq!(h.provider.calls(), 0, "no call reached the provider before consent");
    assert!(h.mail.queries.lock().unwrap().is_empty() && h.mail.read.lock().unwrap().is_empty(), "no mail was read before consent");
    assert_eq!(consent_asks(), 2);

    h.runtime.give_consent("cloud").unwrap();
    assert!(!h.finished(agent.clone(), QUESTION).answer.citations.is_empty());
    h.provider.script(&[SEARCH, ANSWER]);
    h.finished(agent.clone(), QUESTION);
    assert_eq!(consent_asks(), 2, "never asked again after yes");
    assert_eq!(h.provider.calls(), 4);

    let mut other = agent.clone();
    other.provider_id = "cloud-b".into();
    assert!(matches!(h.ask(other, QUESTION), AskOutcome::NeedsConsent { .. }), "a new provider asks again");
    assert_eq!(h.provider.calls(), 4);

    let mut local = agent;
    local.provider_id = "local".into();
    let before = consent_asks();
    h.finished(local, QUESTION);
    assert_eq!(consent_asks(), before, "an on-device model sends nothing off the device, so it does not ask");
    assert_eq!(h.local.calls(), 2);
}

#[test]
fn the_step_cap_stops_a_runaway_loop() {
    let h = Harness::new(&[r#"{"tool":"search","query":"quote invoice payout photos"}"#]);
    h.runtime.give_consent("cloud").unwrap();
    let turn = h.finished(h.agent(&[ReadMail, Search]), QUESTION);
    let cap = usize::try_from(AGENT_MAX_STEPS).unwrap();
    assert!(turn.stopped);
    assert_eq!(turn.steps as usize, cap);
    assert_eq!(h.provider.calls(), cap, "no call past the cap");
    assert_eq!(h.events.count(|e| matches!(e, AgentEvent::Stopped { .. })), 1);
    assert_eq!(h.events.count(|e| matches!(e, AgentEvent::Step { .. })), cap);
    assert!(h.mail.read.lock().unwrap().len() <= usize::try_from(AGENT_CONTEXT_CHUNKS).unwrap(), "excerpts stay within AGENT_CONTEXT_CHUNKS");
    assert!(turn.answer.citations.is_empty());
}

#[test]
fn the_writer_drafts_and_can_neither_label_nor_send() {
    let w = writer("cloud", "fake-model");
    assert!(w.built_in);
    assert_eq!(allowed_words(&w.allowed), vec!["read mail", "search", "draft"]);
    let h = Harness::new(&[
        r#"{"tool":"read_mail","message_ids":["msg-07"]}"#,
        r#"{"tool":"propose_send","draft_id":"d-1"}"#,
        r#"{"tool":"draft","body":"Thursday at 15:00 works for me.","cite":["msg-07"]}"#,
    ]);
    h.runtime.give_consent("cloud").unwrap();
    let turn = h.finished(w, "reply yes to Jonas");
    assert_eq!(turn.answer.kind, AnswerKind::Draft);
    assert_eq!(turn.answer.text, "Thursday at 15:00 works for me.");
    assert_eq!(turn.answer.citations[0].message_id, "msg-07");
    assert!(turn.proposals.is_empty() && h.labels.applied() == 0);
    let first = &h.provider.seen()[0];
    assert!(first.system.starts_with(WRITER_INSTRUCTIONS));
    assert!(!first.system.contains("propose_label") && !first.system.contains("propose_send"));
    assert_eq!((first.model.as_str(), first.max_output_tokens), ("fake-model", MAX_TOKENS));
}

#[test]
fn a_send_waits_for_confirmation() {
    let h = Harness::new(&[r#"{"tool":"propose_send","draft_id":"d-1"}"#, r#"{"answer":"Ready to send.","cite":[]}"#]);
    h.runtime.give_consent("cloud").unwrap();
    let turn = h.finished(h.agent(&[ReadMail, Draft, SendWithConfirmation]), "send my draft");
    let p = &turn.proposals[0];
    assert_eq!(turn.answer.text, "Ready to send.", "an answer that comes with a proposal stands");
    assert_eq!(h.labels.applied(), 0, "nothing sent before approve");
    h.runtime.decide_proposal(&p.id, true).unwrap();
    assert_eq!(*h.labels.applied.lock().unwrap(), vec![contracts::Command::Send { draft_id: "d-1".into() }]);
}

#[test]
fn events_map_onto_the_contract_streams() {
    let h = Harness::new(&[SEARCH, PROPOSE, ANSWER]);
    h.runtime.give_consent("cloud").unwrap();
    let turn = h.finished(h.agent(&[ReadMail, Search, Label]), QUESTION);
    let mapped: Vec<CoreEvent> = h.events.all().iter().filter_map(|e| e.core_event("conv-1")).collect();
    assert!(mapped.contains(&CoreEvent::ProposalReady { proposal_id: turn.proposals[0].id.clone() }));
    assert!(mapped.contains(&CoreEvent::AgentDelta { conversation_id: "conv-1".into(), text: turn.answer.text.clone() }));
    let steps: Vec<u32> = h.events.all().iter().filter_map(|e| if let AgentEvent::Step { number, .. } = e { Some(*number) } else { None }).collect();
    assert_eq!(steps, vec![1, 2, 3]);
}
