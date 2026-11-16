//! One file per step, in the pipeline's order; `retrieve` orchestrates them, `report` reads the result.
pub mod write_questions;
pub mod search_scope;
pub mod gather;
pub mod size_excerpt;
pub mod keep_best;
pub mod clash;
pub mod ai_filter;
pub mod label_line;
pub mod assemble;
pub mod retrieve;
pub mod report;
