//! Folder roles: RFC 6154 SPECIAL-USE attributes first, then the names providers use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FolderRole { Inbox, Sent, Drafts, Trash, Junk, Archive, All, Flagged, Other }

/// How a role was found, for the validation report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleBy { Inbox, SpecialUse, Name, Unknown }

/// `attributes` are the LIST attributes as the server wrote them (for example `\Sent`).
pub fn role_for(name: &str, attributes: &[String]) -> FolderRole {
    role_and_source(name, attributes).0
}

pub fn role_and_source(name: &str, attributes: &[String]) -> (FolderRole, RoleBy) {
    if name.eq_ignore_ascii_case("INBOX") {
        return (FolderRole::Inbox, RoleBy::Inbox);
    }
    for a in attributes {
        let role = match a.to_ascii_lowercase().as_str() {
            "\\sent" => FolderRole::Sent,
            "\\drafts" => FolderRole::Drafts,
            "\\trash" => FolderRole::Trash,
            "\\junk" => FolderRole::Junk,
            "\\archive" => FolderRole::Archive,
            "\\all" => FolderRole::All,
            "\\flagged" => FolderRole::Flagged,
            _ => continue,
        };
        return (role, RoleBy::SpecialUse);
    }
    // No SPECIAL-USE: the leaf name decides (iCloud, many small hosts).
    let leaf = name.rsplit(['/', '.']).next().unwrap_or(name).to_ascii_lowercase();
    let role = match leaf.as_str() {
        "sent" | "sent messages" | "sent items" | "sent mail" => FolderRole::Sent,
        "drafts" | "draft" => FolderRole::Drafts,
        "trash" | "deleted messages" | "deleted items" | "bin" => FolderRole::Trash,
        "junk" | "spam" | "junk email" | "junk mail" => FolderRole::Junk,
        "archive" | "archives" => FolderRole::Archive,
        "all mail" => FolderRole::All,
        _ => return (FolderRole::Other, RoleBy::Unknown),
    };
    (role, RoleBy::Name)
}
