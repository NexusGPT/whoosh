//! The hidden check's import (rig only, `WHOOSH_CORE_CHECK_IMPORT=<a folder standing in for the home folder>`, next to
//! `WHOOSH_CORE_CHECK_FLOW`): the check cannot start an import through a screen that is not built yet, so the rig copy does what the
//! first-start step will do, on a thread of its own: scan (Apple Mail is looked for under `<folder>/Library/Mail`), start the import into
//! Imported, wait for its result, scan again (so `import/sources` shows what the run stored). The check then reads the list and
//! `core state import/sources`. The owner's copy never reads the variable.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::{import_source_id, ImportInto, ImportSourceKind};
use store::Store;

use crate::import_run::ImportBook;

const WAIT: Duration = Duration::from_secs(60);

pub(crate) fn run(book: Arc<ImportBook>, store: Arc<Store>, home: PathBuf) {
    book.set_home(home);
    let spawned = std::thread::Builder::new().name("whoosh-check-import".into()).spawn(move || {
        let id = import_source_id(ImportSourceKind::AppleMail, "~/Library/Mail".to_string());
        if let Err(e) = book.scan(&store, &[]) {
            return eprintln!("check import: the scan was refused: {e}");
        }
        let run = match book.start(&store, &id, ImportInto::Imported) {
            Ok(o) => o.created_id.unwrap_or_default(),
            Err(e) => return eprintln!("check import: the start was refused: {e}"),
        };
        let t0 = Instant::now();
        while t0.elapsed() < WAIT && book.state(&format!("import/{run}/result")).is_none() {
            std::thread::sleep(Duration::from_millis(50));
        }
        if let Err(e) = book.scan(&store, &[]) {
            eprintln!("check import: the second scan was refused: {e}");
        }
    });
    if let Err(e) = spawned {
        eprintln!("check import: its thread did not start: {e}");
    }
}
