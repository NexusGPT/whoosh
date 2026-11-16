//! The six decision pages, in the order a message is decided: an earlier page's answer can be a condition of a later one
//! ("Notify me" reads VIPs and Needs my reply; "Which tab" reads Needs my reply).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Outcome {
    Vips,
    NeedsMyReply,
    SignInCodes,
    WhichTab,
    Labels,
    NotifyMe,
}

impl Outcome {
    pub const ALL: [Outcome; 6] = [Outcome::Vips, Outcome::NeedsMyReply, Outcome::SignInCodes, Outcome::WhichTab, Outcome::Labels, Outcome::NotifyMe];

    /// The key stored in `DecisionModel.decisions_enabled` and used as the prefix of entry ids.
    pub fn key(&self) -> &'static str {
        match self {
            Outcome::Vips => "vips",
            Outcome::NeedsMyReply => "needs-reply",
            Outcome::SignInCodes => "sign-in-codes",
            Outcome::WhichTab => "which-tab",
            Outcome::Labels => "labels",
            Outcome::NotifyMe => "notify-me",
        }
    }

    pub fn from_key(key: &str) -> Option<Outcome> {
        Outcome::ALL.into_iter().find(|o| o.key() == key)
    }

    /// The page title the owner reads.
    pub fn words(&self) -> &'static str {
        match self {
            Outcome::Vips => "VIPs",
            Outcome::NeedsMyReply => "Needs my reply",
            Outcome::SignInCodes => "Sign-in codes",
            Outcome::WhichTab => "Which tab",
            Outcome::Labels => "Labels",
            Outcome::NotifyMe => "Notify me",
        }
    }
}
