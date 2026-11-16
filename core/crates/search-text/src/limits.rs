//! Sizes this crate needs that have no row in docs/CONSTANTS.md yet. They are the contract delta
//! "constants to add" in docs/lanes/core-search-text.md; each is named here once and nowhere else.
//! INDEX_LAG_MS, the only number the brief gives, is read from the ledger by the tests.

/// Memory the tantivy writer may use across its threads while it builds segments.
pub const WRITER_HEAP_BYTES: usize = 200_000_000;
/// Indexing threads of the tantivy writer.
pub const WRITER_THREADS: usize = 2;
/// Rows read from SQLite per round while the writer catches up or rebuilds.
pub const BATCH_ROWS: usize = 5_000;
/// After an update of a message or attachment row, this many of the newest rows are re-checked at once
/// (the store's change feed names the table, not the row; see the finding in docs/lanes/core-search-text.md).
pub const RECHECK_TAIL_ROWS: i64 = 2_000;
/// Hits returned when the caller does not say.
pub const DEFAULT_HITS: usize = 50;
/// Characters of a result snippet.
pub const SNIPPET_CHARS: usize = 160;
/// Rowids compared per window while the index is reconciled with the table; new mail is served between windows.
pub const RECONCILE_WINDOW_ROWIDS: i64 = 50_000;
/// search-forms: index words read per field and segment while one typed word is widened (a bound on the walk). docs/CONSTANTS.md row of the same name.
pub const PREFIX_SCAN_MAX: usize = contracts::constants::PREFIX_SCAN_MAX as usize;
/// search-forms: the weight of the typed word itself over a word that only starts with it, so "consultant" ranks a mail holding
/// "consultant" above one holding only "consultants" at the same strength. docs/CONSTANTS.md row of the same name.
pub const EXACT_WORD_BOOST: f32 = contracts::constants::EXACT_WORD_BOOST as f32;
/// match-inside (SPEC A149): a typed word of at least this many letters matches every index word that HOLDS those letters (the
/// word itself, a word that starts with it, a word that holds it inside: "consultant" finds "brightconsultants" of
/// brightconsultants.example); a shorter word matches only itself. Replaces PREFIX_MIN_CHARS for the owner's search. Row to add to
/// docs/CONSTANTS.md (docs/lanes/match-inside.md); depends on how many index words hold a short run of letters.
pub const SUBSTRING_MIN_CHARS: usize = contracts::constants::SUBSTRING_MIN_CHARS as usize;
/// match-inside: index words read per typed word, across fields and segments, while looking for the words that hold its letters
/// inside (the word list cannot be walked from a start for that, so it is read whole up to this bound). The word itself and the
/// words that start with it are found by a lookup and a ranged walk before this scan, so they are never cut by it. Row to add;
/// depends on the 50 ms answer budget and the size of the word lists (docs/lanes/match-inside.md).
pub const SUBSTRING_SCAN_MAX: usize = contracts::constants::SUBSTRING_SCAN_MAX as usize;
/// match-inside: the most index words one typed word stands for (the word itself first, then the words that start with it, then
/// the words that hold it, each group shortest first). Replaces PREFIX_EXPANSIONS_MAX for the owner's search. Row to add; depends
/// on the 50 ms answer budget (one dictionary lookup and one posting list per word, field and segment).
pub const SUBSTRING_EXPANSIONS_MAX: usize = contracts::constants::SUBSTRING_EXPANSIONS_MAX as usize;
/// match-inside: characters of the passage a text match shows, cut at word boundaries around the first occurrence. Row to add;
/// depends on one line of the search row (SPEC A149 "one line of the text around them").
pub const SEARCH_PASSAGE_CHARS: usize = contracts::constants::SEARCH_PASSAGE_CHARS as usize;
