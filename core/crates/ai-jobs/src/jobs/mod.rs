//! The job tables, grouped by what the owner meets: writing, reading, deciding, learning, reporting.
//! Every prompt is written for this app; none is copied from another product (checks/licence-check.py).
pub(crate) mod decide;
pub(crate) mod learn;
pub(crate) mod read;
pub(crate) mod report;
pub(crate) mod write;
