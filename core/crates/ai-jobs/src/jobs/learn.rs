//! Jobs that learn from mail: the owner's voice, facts, memories, filing.
use crate::schema::*;
use crate::spec::*;

fn entries() -> Schema {
    list("facts found", obj(vec![req("topic", text("what the fact is about")), req("fact", text("the fact, as one sentence"))]))
}

pub(crate) fn specs() -> Vec<JobSpec> {
    vec![
        job(
            JobId::VoiceProfile,
            "Learn your voice",
            "These are messages the owner wrote. Describe how they write so that someone else could write as them: greeting and sign-off habits, sentence length, warmth, formality, typical phrases, how they say yes or no. \
             Describe style only; leave out names, amounts and private facts.",
            vec![messages("sent", "messages written by the owner")],
            obj(vec![req("summary_words", text("a description of the owner's voice, a short paragraph")), req("traits", list("the clearest habits", text("one habit")))]),
        ),
        job(
            JobId::CompactWritingStyle,
            "Merge style notes",
            "Merge these notes about how the owner writes into one short description. Keep what repeats; drop what contradicts the rest.",
            vec![items("notes", "earlier style descriptions")],
            obj(vec![req("summary_words", text("the merged description"))]),
        ),
        job(
            JobId::Persona,
            "Who the owner is in mail",
            "From the owner's sent mail, describe their role, the topics they handle and the kinds of people they write to.",
            vec![messages("sent", "messages written by the owner")],
            obj(vec![req("persona", text("a short paragraph")), req("topics", list("topics they handle", text("one topic")))]),
        ),
        job(
            JobId::ExtractKnowledge,
            "Facts from a document",
            "Read the document and list the lasting facts a person answering mail would need: prices, policies, dates, names, procedures. Skip chatter.",
            vec![mail("document", "the document text")],
            obj(vec![req("entries", entries())]),
        ),
        job(
            JobId::KnowledgeFromHistory,
            "Facts from past mail",
            "Read this mail history and list the lasting facts the owner would need again: who does what, agreed terms, preferences, recurring details.",
            vec![messages("history", "past messages")],
            obj(vec![req("entries", entries())]),
        ),
        job(
            JobId::ReplyMemoriesExtract,
            "Remember from a reply",
            "The owner answered this message. Note what that answer teaches for next time: a preference, a rule of thumb, a fact. One short sentence each. An empty list is allowed.",
            vec![messages("thread", "the conversation with the owner's answer last")],
            obj(vec![req("memories", list("what to remember", text("one sentence")))]),
        ),
        job(
            JobId::SelectReplyMemories,
            "Pick the memories that matter",
            "From the memories given, choose those that help answer this message. Copy them exactly. None is a valid answer.",
            vec![messages("thread", "the message to answer"), items("memories", "what the owner has noted before")],
            obj(vec![req("selected", list("the memories that apply", text("one memory, copied from the list")))]),
        )
        .members("selected[]", "memories"),
        job(
            JobId::ReplyContextQueries,
            "What to look up before replying",
            "Before the owner replies, say which searches of their own mailbox would bring up the facts they need: earlier threads, attachments, prices. Write each search as the words someone would type.",
            vec![messages("thread", "the message to answer")],
            obj(vec![req("queries", list("searches to run, most useful first", text("one search")))]),
        ),
        job(
            JobId::FileAttachment,
            "Where to file an attachment",
            "Suggest where to file this attachment and what to call it, judged by its text and the mail it came with.",
            vec![mail("file_name", "the attachment's file name"), mail("file_text", "the text of the attachment"), messages("context", "the mail it came with").optional()],
            obj(vec![req("label", text("the label or folder to file it under")), req("file_name", text("a clear name for the file")), req("reason", text("one sentence"))]),
        ),
        job(
            JobId::ParseFilingReply,
            "Read the owner's answer on filing",
            "The owner was offered a filing suggestion and answered in words. Say whether they accepted it, and if they changed the label or the name, give the new one.",
            vec![owner("suggestion", "what was suggested"), owner("answer", "what the owner said")],
            obj(vec![req("accepted", flag("whether the owner agreed")), opt("label", text("the label to use, if the owner changed it")), opt("file_name", text("the file name to use, if the owner changed it"))]),
        ),
    ]
}
