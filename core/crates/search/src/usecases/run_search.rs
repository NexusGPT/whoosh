//! One search, end to end: read the chips, ask both indexes, fuse, keep what the filters admit, group by thread,
//! and say which accounts are still loading. Orchestration only: every read goes through a port.
use crate::domain::dates::CivilDate;
use crate::domain::{AccountState, ParsedQuery, SearchOutcome};
use crate::ports::{AccountsPort, AttachmentPort, FactsPort, FilterScanPort, MeaningPort, TextPort};
use crate::usecases::fuse_ranks::{add_ranked_list, fuse_ranks};
use crate::usecases::group_threads::group_threads;
use crate::usecases::parse_query::{parse_query, tokens, ParseContext};
use contracts::constants::SEARCH_DEFAULT_HITS;
use contracts::CoreError;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct SearchRequest<'a> {
    pub text: &'a str,
    /// The owner's today and local offset, for "before June"; the caller reads the clock, never this crate.
    pub today: CivilDate,
    pub utc_offset_min: i32,
}

pub struct Searcher {
    pub text: Arc<dyn TextPort>,
    pub meaning: Arc<dyn MeaningPort>,
    pub facts: Arc<dyn FactsPort>,
    pub scan: Arc<dyn FilterScanPort>,
    pub accounts: Arc<dyn AccountsPort>,
    /// Words inside attachments and their names (contracts delta 10). None: the search reads mail only.
    pub attachments: Option<Arc<dyn AttachmentPort>>,
}

fn bare_words(residual: &str) -> Vec<String> {
    tokens(residual).into_iter().filter(|t| !t.starts_with('"')).collect()
}

impl Searcher {
    /// Read the line without searching (the chips as the owner types).
    pub fn parse(&self, req: &SearchRequest<'_>) -> ParsedQuery {
        let accounts = self.accounts_or_none();
        parse_query(req.text, &ParseContext { today: req.today, utc_offset_min: req.utc_offset_min, accounts: &accounts })
    }

    fn accounts_or_none(&self) -> Vec<AccountState> {
        self.accounts.execute().unwrap_or_else(|e| {
            eprintln!("[search] accounts unavailable, account filters and loading names skipped: {e}");
            Vec::new()
        })
    }

    pub fn run(&self, req: &SearchRequest<'_>) -> Result<SearchOutcome, CoreError> {
        let accounts = self.accounts_or_none();
        let query = parse_query(req.text, &ParseContext { today: req.today, utc_offset_min: req.utc_offset_min, accounts: &accounts });
        let loading: Vec<String> = accounts.iter().filter(|a| !a.indexed).map(|a| a.name.clone()).collect();
        let mut out = SearchOutcome { offer_server_search: !loading.is_empty(), accounts_still_loading: loading, ..SearchOutcome::default() };
        let limit = SEARCH_DEFAULT_HITS as usize;
        let words = bare_words(&query.residual);
        let mut snippets: HashMap<String, String> = HashMap::new();
        let mut exact: HashSet<String> = HashSet::new();
        let mut attached: HashMap<String, contracts::SearchHit> = HashMap::new();
        let fused = if query.residual.is_empty() {
            if query.filters.is_empty() {
                out.query = query;
                return Ok(out);
            }
            let ids = self.scan.execute(&query.filters, limit)?;
            fuse_ranks(&ids, &[])
        } else {
            let text_hits = self.text.execute(&query.residual, limit)?;
            let meaning_words = query.residual.replace('"', " ");
            let meaning_hits = match self.meaning.execute(meaning_words.trim(), limit) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("[search] meaning search failed, text results only: {e}");
                    out.meaning_unavailable = Some(e.to_string());
                    Vec::new()
                }
            };
            if !query.phrases.is_empty() {
                exact = text_hits.iter().map(|h| h.message_id.clone()).collect();
            }
            for h in &text_hits {
                snippets.entry(h.message_id.clone()).or_insert_with(|| h.snippet.clone());
            }
            let t: Vec<String> = text_hits.into_iter().map(|h| h.message_id).collect();
            let m: Vec<String> = meaning_hits.into_iter().map(|h| h.message_id).collect();
            let fused = fuse_ranks(&t, &m);
            // exact phrases are the text index's alone; bare words also ask the attachments
            match (&self.attachments, query.phrases.is_empty()) {
                (Some(port), true) => match port.execute(meaning_words.trim(), limit) {
                    Ok(hits) => {
                        let ids: Vec<String> = hits.iter().map(|h| h.message_id.clone()).collect();
                        for h in hits {
                            attached.entry(h.message_id.clone()).or_insert(h);
                        }
                        add_ranked_list(fused, &ids)
                    }
                    Err(e) => {
                        eprintln!("[search] attachment search failed, mail results only: {e}");
                        fused
                    }
                },
                _ => fused,
            }
        };
        let ids: Vec<String> = fused.iter().map(|f| f.message_id.clone()).collect();
        let facts: HashMap<String, contracts::Message> =
            self.facts.execute(&ids)?.into_iter().filter(|m| query.filters.admits(m)).map(|m| (m.id.clone(), m)).collect();
        exact.retain(|id| facts.contains_key(id));
        out.no_exact_match = !query.phrases.is_empty() && exact.is_empty();
        let mut terms = query.phrases.clone();
        terms.extend(words);
        out.threads = group_threads(&fused, &facts, &snippets, &exact, &terms);
        for m in out.threads.iter_mut().flat_map(|t| t.matches.iter_mut()) {
            m.attachment = attached.remove(&m.message_id);
        }
        out.query = query;
        Ok(out)
    }
}
