//! The Apple Mail tree: `<mail dir>/V*/<account>/<mailbox>.mbox/**/Messages/*.emlx`. Nested mailboxes are nested `.mbox` folders and
//! join as "Parent/Child". Folders and files are read in name order, one message file at a time.
use std::fs;
use std::path::{Path, PathBuf};

use crate::consts::IMPORT_MAX_MESSAGE_BYTES;
use crate::emlx::{self, file_modified_ms, message_number, partial_attachments};
use crate::error::{classify_io_error, ImportError};
use crate::model::{MailVisitor, RawMail, Source};

/// A mailbox tree is a few folders deep; this stops a loop of folders from running for ever.
const MAX_DEPTH: usize = 16;
/// Where Mail keeps no messages.
const NOT_MAILBOXES: [&str; 4] = ["Attachments", "MailData", "Bundles", "Signatures"];
/// The name given to messages found in a Messages folder that sits under no `.mbox` folder.
const NO_MAILBOX_NAME: &str = "Mail";

/// ~/Library/Mail under `home`: Ok(None) when Apple Mail has never run here, Err(NeedsFullDiskAccess) when macOS refuses the listing.
pub fn find_apple_mail(home: &Path) -> Result<Option<Source>, ImportError> {
    let dir = home.join("Library").join("Mail");
    match fs::read_dir(&dir) {
        Ok(_) => Ok(Some(Source::AppleMail { mail_dir: dir })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(classify_io_error(&dir, &e)),
    }
}

fn sorted_dirs(dir: &Path) -> Result<Vec<(String, PathBuf)>, ImportError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| classify_io_error(dir, &e))? {
        let entry = entry.map_err(|e| classify_io_error(dir, &e))?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            out.push((entry.file_name().to_string_lossy().into_owned(), entry.path()));
        }
    }
    out.sort();
    Ok(out)
}

fn emlx_files(dir: &Path) -> Result<Vec<PathBuf>, ImportError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| classify_io_error(dir, &e))? {
        let entry = entry.map_err(|e| classify_io_error(dir, &e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".emlx") && entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            out.push(entry.path());
        }
    }
    out.sort();
    Ok(out)
}

fn is_version_dir(name: &str) -> bool {
    name.len() > 1 && name.starts_with('V') && name[1..].chars().all(|c| c.is_ascii_digit())
}

type FileFn<'a> = dyn FnMut(&str, &Path, &Path) -> Result<(), ImportError> + 'a;

/// `f(mailbox, Messages folder, message file)` for every .emlx file, mailbox by mailbox.
fn walk_files(mail_dir: &Path, f: &mut FileFn<'_>) -> Result<(), ImportError> {
    for (name, version) in sorted_dirs(mail_dir)? {
        if !is_version_dir(&name) {
            continue;
        }
        for (account_name, account) in sorted_dirs(&version)? {
            if NOT_MAILBOXES.contains(&account_name.as_str()) {
                continue;
            }
            walk_dir(&account, &mut Vec::new(), 0, f)?;
        }
    }
    Ok(())
}

fn walk_dir(dir: &Path, chain: &mut Vec<String>, depth: usize, f: &mut FileFn<'_>) -> Result<(), ImportError> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    for (name, path) in sorted_dirs(dir)? {
        if let Some(stem) = name.strip_suffix(".mbox") {
            chain.push(stem.to_string());
            walk_dir(&path, chain, depth + 1, f)?;
            chain.pop();
        } else if name == "Messages" {
            let mailbox = if chain.is_empty() { NO_MAILBOX_NAME.to_string() } else { chain.join("/") };
            for file in emlx_files(&path)? {
                f(&mailbox, &path, &file)?;
            }
        } else if !NOT_MAILBOXES.contains(&name.as_str()) {
            walk_dir(&path, chain, depth + 1, f)?;
        }
    }
    Ok(())
}

/// How many message files each mailbox holds, from the folder listings only (no file is opened). In walk order.
pub fn count_apple_mail(mail_dir: &Path) -> Result<Vec<(String, u64)>, ImportError> {
    let mut counts: Vec<(String, u64)> = Vec::new();
    walk_files(mail_dir, &mut |mailbox: &str, _: &Path, _: &Path| {
        if let Some(last) = counts.last_mut() {
            if last.0 == mailbox {
                last.1 += 1;
                return Ok(());
            }
        }
        counts.push((mailbox.to_string(), 1));
        Ok(())
    })?;
    Ok(counts)
}

/// Hand every message of the tree to the visitor, one file at a time.
pub fn walk_apple_mail(mail_dir: &Path, visitor: &mut dyn MailVisitor) -> Result<(), ImportError> {
    walk_files(mail_dir, &mut |mailbox: &str, messages_dir: &Path, file: &Path| read_one(mailbox, messages_dir, file, visitor))
}

fn read_one(mailbox: &str, messages_dir: &Path, file: &Path, v: &mut dyn MailVisitor) -> Result<(), ImportError> {
    let origin = file.display().to_string();
    let len = match fs::metadata(file) {
        Ok(m) => m.len(),
        Err(e) => {
            v.skip(mailbox, origin, format!("could not be read: {e}"));
            return Ok(());
        }
    };
    if len > IMPORT_MAX_MESSAGE_BYTES {
        v.skip(mailbox, origin, format!("is {len} bytes, over the {IMPORT_MAX_MESSAGE_BYTES} byte limit for one message"));
        return Ok(());
    }
    let data = match fs::read(file) {
        Ok(d) => d,
        Err(e) => {
            v.skip(mailbox, origin, format!("could not be read: {e}"));
            return Ok(());
        }
    };
    let parsed = match emlx::parse_emlx(data) {
        Ok(p) => p,
        Err(words) => {
            v.skip(mailbox, origin, words);
            return Ok(());
        }
    };
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let attachments = if name.ends_with(".partial.emlx") { partial_attachments(messages_dir, message_number(name)) } else { Vec::new() };
    v.mail(RawMail {
        mailbox: mailbox.to_string(),
        bytes: parsed.message,
        flags: parsed.flags,
        received_ms: parsed.received_ms.or_else(|| file_modified_ms(file)),
        attachments,
        origin,
    })
}
