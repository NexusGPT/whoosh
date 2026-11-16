//! Jobs that put words in the composer: drafts, suggested replies, rewrites, dictation, translation.
use crate::schema::*;
use crate::spec::*;

pub(crate) fn specs() -> Vec<JobSpec> {
    let voice = || owner("voice", "how the owner writes, from their sent mail").optional();
    let instructions = || owner("instructions", "the owner's standing writing instructions").optional();
    vec![
        job(
            JobId::DraftReply,
            "Draft in your voice",
            "Write the message the owner wants to send. Say what the intent says, in the owner's own voice, and follow the standing writing instructions. \
             If a thread is given, write the reply to its last message. Write the message body only: no subject line, no sign-off block unless the voice includes one, no remarks about the draft.",
            vec![owner("intent", "what the owner wants to say"), messages("thread", "the conversation so far").optional(), voice(), instructions()],
            obj(vec![req("text", text("the message body, ready to send"))]),
        ),
        job(
            JobId::SuggestedReplies,
            "Suggested replies",
            "Offer short replies the owner could send to the last message of the thread. The replies must differ in what they decide, not only in wording: for example accept, decline, ask for more. \
             Each reply has a label of a few words for a button and the full text of the reply.",
            vec![messages("thread", "the conversation"), count("count", "how many replies to offer"), voice(), instructions()],
            obj(vec![req("replies", list_max("the replies", obj(vec![req("label", text("a few words for a button")), req("text", text("the full reply"))]), "count"))]),
        ),
        job(
            JobId::Rewrite,
            "Rewrite a paragraph",
            "Rewrite the paragraph the way the style asks. Keep every fact, name, number and date. Return the rewritten paragraph only.",
            vec![mail("paragraph", "the paragraph to rewrite"), owner("style", "how to change it, for example shorter, warmer, more formal"), voice(), instructions()],
            obj(vec![req("text", text("the rewritten paragraph"))]),
        ),
        job(
            JobId::DictationCleanup,
            "Clean up dictation",
            "The text was spoken and typed by a speech engine. Add punctuation and capital letters, remove filler words and false starts, fix words the engine plainly misheard. \
             Do not add, remove or reorder ideas and do not answer the text.",
            vec![mail("spoken", "the words as the speech engine typed them"), mail("language", "the language spoken, when known").optional()],
            obj(vec![req("text", text("the cleaned text"))]),
        ),
        job(
            JobId::Translate,
            "Translate",
            "Translate the text into the target language. Keep names, numbers, links and the paragraph breaks. Say which language the text was written in.",
            vec![mail("text", "the text to translate"), owner("target_language", "the language to translate into")],
            obj(vec![req("text", text("the translation")), req("source_language", text("the language of the original, in English"))]),
        ),
        job(
            JobId::ComposeAutocomplete,
            "Finish the sentence",
            "Continue the owner's text from where it stops. Offer a short continuation of the sentence in progress, not a new paragraph. Start exactly where the text ends, with a space if one is needed.",
            vec![mail("before_cursor", "everything the owner has typed so far"), messages("thread", "the message being answered").optional(), voice()],
            obj(vec![req("continuation", raw("the words to add, with a leading space when the sentence needs one"))]),
        ),
        job(
            JobId::DraftFollowUp,
            "Draft a follow-up",
            "The owner wrote last and got no answer. Write a short, polite follow-up that points back to the open question without blaming the recipient.",
            vec![messages("thread", "the conversation, owner's message last"), voice(), instructions()],
            obj(vec![req("text", text("the follow-up body"))]),
        ),
        job(
            JobId::ReplyNudge,
            "Why this waits for you",
            "This thread is waiting for the owner to answer. Write one line that tells the owner what is being asked of them, so they can decide at a glance.",
            vec![messages("thread", "the conversation, the other person's message last")],
            obj(vec![req("nudge", text("one line, plain words"))]),
        ),
        job(
            JobId::CheckInMessage,
            "Check-in message",
            "Write a short message asking the other person how a thing stands. The purpose says what the thing is.",
            vec![owner("purpose", "what the owner wants to check on"), owner("recipient_name", "who the message is for").optional(), voice()],
            obj(vec![req("text", text("the message body"))]),
        ),
        job(
            JobId::DraftAttachmentSelection,
            "Pick attachments",
            "Choose which of the owner's files, if any, belong with the reply to this thread. Pick only names from the list. An empty answer is allowed.",
            vec![messages("thread", "the conversation"), items("attachments", "the file names the owner could attach")],
            obj(vec![req("attachments", list("names of the files to attach", text("one file name from the list")))]),
        )
        .members("attachments[]", "attachments"),
    ]
}
