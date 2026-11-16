//! Jobs that read mail for the owner: summaries, codes, status, briefs.
use crate::schema::*;
use crate::spec::*;

pub(crate) fn specs() -> Vec<JobSpec> {
    vec![
        job(
            JobId::SummariseThread,
            "Summarise a thread",
            "Summarise the conversation for someone who has not read it. Lead with what was decided or asked. Keep names, amounts and dates exact. Write in the language of the thread unless the language field says otherwise.",
            vec![messages("thread", "the conversation, oldest first"), owner("language", "the language to write in").optional()],
            obj(vec![req("summary", text("two or three sentences")), req("key_points", list("the points to remember", text("one point"))), req("open_questions", list("questions still unanswered, empty when none", text("one question")))]),
        ),
        job(
            JobId::DigestLine,
            "One line for the digest",
            "Say what this message is about in one line a reader can scan in a list.",
            vec![messages("message", "the message")],
            obj(vec![req("line", text("one line"))]),
        ),
        job(
            JobId::ExtractSignInCode,
            "Find the sign-in code",
            "Find the one-time sign-in or verification code in this message, and the sign-in link if the message has one. Copy them exactly. If there is none, say so; never invent a code.",
            vec![mail("message", "the message text")],
            obj(vec![req("found", flag("whether the message holds a code or a sign-in link")), opt("code", text("the code, exactly as written")), opt("link", text("the sign-in link, exactly as written"))]),
        )
        .copied("code", "message")
        .copied("link", "message"),
        job(
            JobId::ClassifyNeedsReply,
            "Does this need a reply",
            "Say where the thread stands for the owner. needs_reply: someone is waiting for the owner's answer. awaiting_reply: the owner wrote last and waits. fyi: nothing is asked of the owner. done: the matter is closed.",
            vec![messages("thread", "the conversation, oldest first"), owner("owner_address", "the owner's address, so you know which messages are theirs")],
            obj(vec![req("status", pick("where the thread stands", &["needs_reply", "awaiting_reply", "fyi", "done"])), req("reason", text("one sentence"))]),
        ),
        job(
            JobId::MeetingBrief,
            "Brief for a meeting",
            "Prepare the owner for a meeting from the mail given. Say who is coming and what each one wants, what was agreed before, and what is still open. Use only what the mail says.",
            vec![messages("thread", "the mail around the meeting"), items("attendees", "the people in the meeting").optional()],
            obj(vec![req("brief", text("a short paragraph")), req("points", list("what to raise or settle", text("one point")))]),
        ),
        job(
            JobId::AvailabilityReply,
            "Answer with my free times",
            "The sender asks to meet. Reply with times taken only from the owner's free slots. Propose the slots that best fit what the sender asked.",
            vec![messages("thread", "the conversation"), items("free_slots", "times the owner is free, as written for the reader"), voice_opt()],
            obj(vec![req("text", text("the reply body")), req("proposed_slots", list("the slots named in the reply", text("one slot, copied from the list")))]),
        )
        .members("proposed_slots[]", "free_slots"),
        job(
            JobId::FindSnippets,
            "Find a saved snippet",
            "Choose the saved snippets that answer this message. Name them as they appear in the list. None is a valid answer.",
            vec![messages("thread", "the message being answered"), items("snippets", "saved snippets, each as name: text")],
            obj(vec![req("matches", list("snippet names, best first", text("one snippet name")))]),
        )
        .members("matches[]", "snippets"),
    ]
}

fn voice_opt() -> InField {
    owner("voice", "how the owner writes, from their sent mail").optional()
}
