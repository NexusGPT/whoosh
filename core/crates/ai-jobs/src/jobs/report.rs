//! Chat upkeep and the mailbox report: seven jobs that together make one report.
use crate::schema::*;
use crate::spec::*;

fn findings() -> Schema {
    list("findings", text("one finding, with the figure it rests on"))
}

pub(crate) fn specs() -> Vec<JobSpec> {
    vec![
        job(
            JobId::CompactChat,
            "Shorten a long chat",
            "Condense this conversation with the assistant into notes that let it continue: what was asked, what was done, what is still open.",
            vec![messages("conversation", "the chat so far")],
            obj(vec![req("summary", text("the notes"))]),
        ),
        job(
            JobId::ChatMemory,
            "Remember from a chat",
            "From this chat, note what the owner told the assistant that should hold in later chats: preferences, corrections, standing facts. An empty list is allowed.",
            vec![messages("conversation", "the chat")],
            obj(vec![req("memories", list("what to remember", text("one sentence")))]),
        ),
        job(
            JobId::ReportBehavior,
            "Report: how you use mail",
            "From these mailbox figures, describe how the owner handles mail: volume, timing, what they answer, what they let sit.",
            vec![mail("figures", "counts and timings from the mailbox")],
            obj(vec![req("findings", findings())]),
        ),
        job(
            JobId::ReportLabels,
            "Report: label use",
            "From these label figures, say which labels earn their place and which overlap or sit unused, and what to merge or drop.",
            vec![mail("figures", "label counts and overlaps"), items("labels", "the label names")],
            obj(vec![req("suggestions", list("label changes", obj(vec![req("label", text("the label")), req("advice", text("what to do and why"))])))]),
        ),
        job(
            JobId::ReportResponsePatterns,
            "Report: how fast you answer",
            "From these response figures, describe who gets quick answers, who waits, and where the owner's answers slow down.",
            vec![mail("figures", "response times by sender and by hour")],
            obj(vec![req("findings", findings())]),
        ),
        job(
            JobId::ReportEmailSummary,
            "Report: what the mail was about",
            "Summarise what this period's mail was about, in topics, with the weight each had.",
            vec![messages("messages", "a sample of the period's mail")],
            obj(vec![req("summary", text("a short paragraph")), req("topics", list("topics, heaviest first", text("one topic")))]),
        ),
        job(
            JobId::ReportPersona,
            "Report: the owner as a correspondent",
            "From the figures and mail given, describe the owner as a correspondent: how they present, who they serve, what they are known for in the mailbox.",
            vec![mail("figures", "counts and timings"), messages("sent", "a sample of the owner's sent mail")],
            obj(vec![req("persona", text("a short paragraph"))]),
        ),
        job(
            JobId::ReportExecutiveSummary,
            "Report: the short version",
            "Write the executive summary of the report from its findings: the three things the owner should know first.",
            vec![items("findings", "all findings of the report")],
            obj(vec![req("summary", text("a short paragraph")), req("headlines", list("the first things to know", text("one headline")))]),
        ),
        job(
            JobId::ReportRecommendations,
            "Report: what to do next",
            "From the findings, recommend actions the owner can take this week. Each action says why, and rests on a finding.",
            vec![items("findings", "all findings of the report")],
            obj(vec![req("recommendations", list("actions, most useful first", obj(vec![req("action", text("what to do")), req("why", text("the finding it rests on"))])))]),
        )
    ]
}
