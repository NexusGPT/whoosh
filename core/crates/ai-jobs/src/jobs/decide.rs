//! Jobs that decide something about mail: rules, senders, cold mail, clean-up.
use crate::schema::*;
use crate::spec::*;

pub(crate) fn specs() -> Vec<JobSpec> {
    vec![
        job(
            JobId::MatchRule,
            "Which rule applies",
            "Pick the one rule, if any, whose words describe this message. Name it exactly as listed. If no rule fits, leave the rule out.",
            vec![messages("message", "the message"), items("rules", "the owner's rules, each as name: what it covers")],
            obj(vec![opt("rule", text("the name of the rule that applies")), req("reason", text("one sentence"))]),
        )
        .members("rule", "rules"),
        job(
            JobId::RuleArguments,
            "Fill in a rule's fields",
            "A rule applies to this message. Read the message and fill in each field the rule asks for. Leave a field out when the message does not say.",
            vec![messages("message", "the message"), owner("rule", "the rule that applies"), items("fields", "the fields to fill, each as name: what it means")],
            obj(vec![req("arguments", list("the fields found", obj(vec![req("name", text("the field name, as listed")), req("value", text("the value from the message"))])))]),
        )
        .members("arguments[].name", "fields"),
        job(
            JobId::ColdEmail,
            "Cold email check",
            "Decide whether this is an unsolicited message from a stranger selling or asking for something, judged by the policy. Mail from people the owner already deals with is not cold.",
            vec![messages("message", "the message"), owner("policy", "what the owner counts as cold mail").optional()],
            obj(vec![req("is_cold", flag("whether the message is cold mail")), req("reason", text("one sentence"))]),
        ),
        job(
            JobId::RecurringPattern,
            "Spot a repeating pattern",
            "These are recent messages from the same sender. Say whether they repeat in a way the owner deals with the same way each time, and if so write that as a rule in plain words.",
            vec![messages("recent_mail", "recent mail from one sender, newest first")],
            obj(vec![req("repeats", flag("whether the mail follows a pattern")), opt("pattern", text("what repeats")), opt("rule_words", text("a rule in plain words"))]),
        ),
        job(
            JobId::CategorizeSender,
            "Categorise a sender",
            "Put this sender in one category from the list, judged by what they send. Use the category name exactly as listed.",
            vec![mail("sender", "the sender's name and address"), messages("samples", "a few messages from them"), items("categories", "the categories, each as name: what it holds")],
            obj(vec![req("category", text("the category name")), req("reason", text("one sentence"))]),
        )
        .members("category", "categories"),
        job(
            JobId::CategorizeSenders,
            "Categorise many senders",
            "Put each sender in one category from the list, judged by the line given for them. Answer for every sender, once each.",
            vec![items("senders", "senders, each as address: a sample subject"), items("categories", "the categories, each as name: what it holds")],
            obj(vec![req("assignments", list("one entry per sender", obj(vec![req("sender", text("the address")), req("category", text("the category name"))])))]),
        )
        .members("assignments[].category", "categories")
        .members("assignments[].sender", "senders"),
        job(
            JobId::SenderPattern,
            "What this sender does",
            "Describe what this sender usually sends and whether the owner could treat all of it the same way.",
            vec![mail("sender", "the sender's name and address"), messages("samples", "messages from them")],
            obj(vec![req("pattern", text("what they usually send")), req("same_treatment", flag("whether one action fits all their mail")), opt("suggested_rule", text("a rule in plain words"))]),
        ),
        job(
            JobId::PromptToRules,
            "Turn words into rules",
            "The owner described how they want mail handled. Write one rule for each separate wish: when it applies, and what to do.",
            vec![owner("words", "the owner's description")],
            obj(vec![req("rules", list("one rule per wish", obj(vec![req("name", text("a short name")), req("when", text("the condition")), req("then", text("the action"))])))]),
        ),
        job(
            JobId::CleanInbox,
            "Keep or archive",
            "Decide whether the owner still needs this message in the inbox. Keep what is waiting for them or may be needed soon; archive what is finished or only informative.",
            vec![messages("message", "the message"), owner("policy", "what the owner wants kept").optional()],
            obj(vec![req("action", pick("what to do with it", &["keep", "archive"])), req("reason", text("one sentence"))]),
        ),
    ]
}
