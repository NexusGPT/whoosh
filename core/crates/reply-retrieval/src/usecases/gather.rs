//! Step 2: one search per question, at most `excerpts_per_question` each, then the hits merged. An excerpt from a source outside the scope is
//! dropped here (the index leaked it; the writer never sees what lies behind the account wall). A chunk found by several questions becomes ONE
//! candidate that remembers every question that found it and keeps its best score. A failed search costs its question only.
use crate::domain::{DropReason, Dropped, ExcerptQuery, FoundExcerpt, QuestionTrace};
use crate::ports::ExcerptIndex;
use contracts::ReplySourceRef;

/// One excerpt on its way to the writer, with the indices of the questions that found it.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub found: FoundExcerpt,
    pub found_by: Vec<usize>,
}

/// What step 2 hands on and what it reports.
#[derive(Debug, Clone, PartialEq)]
pub struct Gathered {
    pub candidates: Vec<Candidate>,
    pub per_question: Vec<QuestionTrace>,
    pub hits: u32,
    pub merged: u32,
    pub dropped: Vec<Dropped>,
}

/// Runs the searches. `per_question` is the count already clamped to its ceiling.
pub fn gather(index: &dyn ExcerptIndex, questions: &[String], scope: &[ReplySourceRef], per_question: usize) -> Gathered {
    let mut g = Gathered { candidates: vec![], per_question: vec![], hits: 0, merged: 0, dropped: vec![] };
    for (qi, question) in questions.iter().enumerate() {
        let query = ExcerptQuery { text: question.clone(), sources: scope.to_vec(), limit: per_question as u32 };
        let mut list = match index.execute(&query) {
            Ok(list) => list,
            Err(e) => {
                g.per_question.push(QuestionTrace { question: question.clone(), found: 0, search_failed: Some(e.to_string()) });
                continue;
            }
        };
        list.sort_by(|a, b| b.score.total_cmp(&a.score));
        list.truncate(per_question);
        g.per_question.push(QuestionTrace { question: question.clone(), found: list.len() as u32, search_failed: None });
        g.hits += list.len() as u32;
        for hit in list {
            if !scope.contains(&hit.source) {
                g.dropped.push(Dropped { chunk_id: hit.chunk_id, reason: DropReason::OutsideScope });
                continue;
            }
            match g.candidates.iter_mut().find(|c| c.found.chunk_id == hit.chunk_id) {
                Some(c) => {
                    g.merged += 1;
                    if hit.score > c.found.score {
                        c.found.score = hit.score;
                    }
                    if !c.found_by.contains(&qi) {
                        c.found_by.push(qi);
                    }
                }
                None => g.candidates.push(Candidate { found: hit, found_by: vec![qi] }),
            }
        }
    }
    g
}
