//! An Outlook for Mac export (`.olm`): a zip archive. Each message is one XML file named `message_NNNNN.xml` somewhere under a
//! `com.microsoft.__Messages` folder whose sub-folders name the mailbox; attachments are other entries of the archive, named by
//! the message XML. Entries are read one at a time and a message's attachments are only measured, never opened.
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use zip::ZipArchive;

use crate::consts::IMPORT_MAX_MESSAGE_BYTES;
use crate::error::{classify_io_error, ImportError};
use crate::model::{AttachmentRef, MailVisitor, RawMail};
use crate::olm_build;

const MESSAGES_FOLDER: &str = "com.microsoft.__Messages";
/// The mailbox name for a message XML found outside any sub-folder of the messages folder.
const NO_MAILBOX_NAME: &str = "Outlook";

/// `message_` + digits + `.xml` as the last path part.
pub fn is_message_entry(name: &str) -> bool {
    let file = name.rsplit('/').next().unwrap_or(name);
    match file.strip_prefix("message_").and_then(|r| r.strip_suffix(".xml")) {
        Some(digits) => !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

/// The folders between `com.microsoft.__Messages` and the file, joined by "/".
pub fn mailbox_of(entry: &str) -> String {
    let parts: Vec<&str> = entry.split('/').collect();
    match parts.iter().position(|p| *p == MESSAGES_FOLDER) {
        Some(i) if i + 2 < parts.len() => parts[i + 1..parts.len() - 1].join("/"),
        _ => NO_MAILBOX_NAME.to_string(),
    }
}

fn open_archive(file: &Path) -> Result<ZipArchive<BufReader<File>>, ImportError> {
    let handle = File::open(file).map_err(|e| classify_io_error(file, &e))?;
    ZipArchive::new(BufReader::new(handle))
        .map_err(|e| ImportError::Unreadable { path: file.to_path_buf(), words: format!("{} is not a readable Outlook for Mac archive: {e}", file.display()) })
}

/// How many messages the archive holds, from its directory of entries alone.
pub fn count_olm(file: &Path) -> Result<u64, ImportError> {
    let archive = open_archive(file)?;
    Ok(archive.file_names().filter_map(|n| n.ok()).filter(|n| is_message_entry(n)).count() as u64)
}

/// Read one entry as text. Ok(None) when it is over the size limit (the limit is checked on what is read, not on the header's claim).
fn read_entry(archive: &mut ZipArchive<BufReader<File>>, name: &str) -> Result<Option<String>, String> {
    let mut entry = archive.by_name(name).map_err(|e| format!("could not be opened: {e}"))?;
    let mut bytes = Vec::new();
    (&mut entry).take(IMPORT_MAX_MESSAGE_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("could not be read: {e}"))?;
    if bytes.len() as u64 > IMPORT_MAX_MESSAGE_BYTES {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

fn measured(archive: &mut ZipArchive<BufReader<File>>, mut a: AttachmentRef) -> AttachmentRef {
    a.size = archive.by_name(&a.location).map(|f| f.size()).unwrap_or(0);
    a
}

/// Hand every message of the archive to the visitor, folder by folder, one entry at a time.
pub fn walk_olm(file: &Path, visitor: &mut dyn MailVisitor) -> Result<(), ImportError> {
    let mut archive = open_archive(file)?;
    // zip 9: file_names() yields Result<Cow<str>, ZipError> (a name that is not valid text is skipped, never a crash)
    let mut names: Vec<String> = archive.file_names().filter_map(|n| n.ok()).filter(|n| is_message_entry(n)).map(|n| n.into_owned()).collect();
    names.sort();
    for name in names {
        let mailbox = mailbox_of(&name);
        let origin = format!("{} ({name})", file.display());
        let xml = match read_entry(&mut archive, &name) {
            Ok(Some(x)) => x,
            Ok(None) => {
                visitor.skip(&mailbox, origin, format!("is over the {IMPORT_MAX_MESSAGE_BYTES} byte limit for one message"));
                continue;
            }
            Err(words) => {
                visitor.skip(&mailbox, origin, words);
                continue;
            }
        };
        match olm_build::build(&xml) {
            Err(words) => visitor.skip(&mailbox, origin, words),
            Ok(built) => {
                let attachments = built.attachments.into_iter().map(|a| measured(&mut archive, a)).collect();
                visitor.mail(RawMail { mailbox, bytes: built.bytes, flags: built.flags, received_ms: built.received_ms, attachments, origin })?;
            }
        }
    }
    Ok(())
}
