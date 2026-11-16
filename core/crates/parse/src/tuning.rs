//! Every tunable of the parse crate, in one place. docs/CONSTANTS.md is read-only to this lane, so
//! these are listed under "## Constants to add" in docs/lanes/core-parse.md for the root to move
//! into the ledger. CODE_EXTRACT_MIN is not here: the tests read it from docs/CONSTANTS.md.

/// Shortest and longest run of digits taken as a one-time code or verification number.
pub const CODE_DIGITS_MIN: usize = 4;
pub const CODE_DIGITS_MAX: usize = 8;
/// Shortest and longest run of capitals and digits taken as an alphanumeric code.
pub const CODE_ALNUM_MIN: usize = 5;
pub const CODE_ALNUM_MAX: usize = 8;
/// How far back from a candidate the cue words are read, in characters.
pub const CUE_WINDOW_CHARS: usize = 90;
/// How far back and forward the "not a login code" words (promo, postal, order...) are read.
pub const NEGATIVE_BEFORE_CHARS: usize = 40;
pub const NEGATIVE_AFTER_CHARS: usize = 24;
/// A link counts as a sign-in link only when it carries an opaque token at least this long.
pub const LINK_TOKEN_MIN_CHARS: usize = 16;
/// Sign-in links reported per message.
pub const LINKS_MAX: usize = 3;
/// Characters of visible text kept for the list preview.
pub const PREVIEW_CHARS: usize = 140;
/// Characters of visible text handed to the decision contract as the message state.
pub const DECISION_TEXT_CHARS: usize = 2000;
/// Lines joined when an attribution line ("On ... wrote:") wraps.
pub const ATTRIBUTION_WRAP_LINES: usize = 3;
/// Lines searched after a "From:" line for the Sent/Subject lines of a quoted header block.
pub const QUOTE_HEADER_LINES: usize = 6;
