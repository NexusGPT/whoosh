//! The index schema and its analyzer. Words are split on anything that is not a letter or digit,
//! lower-cased and folded to ASCII (so "résumé" finds "resume"). Positions are kept, which is what
//! makes an exact quoted phrase possible.
use tantivy::schema::{Field, IndexRecordOption, NumericOptions, Schema, TextFieldIndexing, TextOptions, STORED, STRING};
use tantivy::tokenizer::{AsciiFoldingFilter, LowerCaser, RemoveLongFilter, SimpleTokenizer, TextAnalyzer};
use tantivy::Index;

pub const ANALYZER: &str = "whoosh_text";
/// A word longer than this many bytes is not indexed (tantivy's own default guard).
const LONGEST_WORD: usize = 40;

#[derive(Clone, Copy)]
pub struct Fields {
    pub id: Field,
    pub rowid: Field,
    pub stamp: Field,
    pub subject: Field,
    pub from: Field,
    pub to: Field,
    pub body: Field,
    pub attachments: Field,
}

impl Fields {
    /// The fields a bare query word is searched in.
    pub fn searchable(&self) -> Vec<Field> {
        vec![self.subject, self.from, self.to, self.body, self.attachments]
    }
}

pub fn build() -> (Schema, Fields) {
    let mut b = Schema::builder();
    let text = TextOptions::default().set_indexing_options(TextFieldIndexing::default().set_tokenizer(ANALYZER).set_index_option(IndexRecordOption::WithFreqsAndPositions));
    let id = b.add_text_field("id", STRING | STORED);
    let rowid = b.add_u64_field("rowid", NumericOptions::default().set_indexed().set_fast());
    let stamp = b.add_u64_field("stamp", NumericOptions::default().set_fast());
    let subject = b.add_text_field("subject", text.clone());
    let from = b.add_text_field("from", text.clone());
    let to = b.add_text_field("to", text.clone());
    let body = b.add_text_field("body", text.clone());
    let attachments = b.add_text_field("attachments", text);
    (b.build(), Fields { id, rowid, stamp, subject, from, to, body, attachments })
}

pub fn register_analyzer(index: &Index) {
    let analyzer = TextAnalyzer::builder(SimpleTokenizer::default()).filter(RemoveLongFilter::limit(LONGEST_WORD)).filter(LowerCaser).filter(AsciiFoldingFilter).build();
    index.tokenizers().register(ANALYZER, analyzer);
}
