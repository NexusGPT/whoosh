mod support;

use std::io;

use import::{classify_io_error, find_apple_mail, import, is_permission_refusal, AccountAddress, ImportContext, ImportError, Source};
use support::{VecSink, IMPORTED};

#[test]
fn a_permission_refusal_is_a_typed_result_with_the_words_the_step_shows() {
    let path = std::path::Path::new("/Users/someone/Library/Mail");
    for e in [io::Error::from(io::ErrorKind::PermissionDenied), io::Error::from_raw_os_error(1), io::Error::from_raw_os_error(13)] {
        assert!(is_permission_refusal(&e), "{e:?}");
        match classify_io_error(path, &e) {
            ImportError::NeedsFullDiskAccess { path: p, words } => {
                assert_eq!(p, path);
                assert!(words.contains("Full Disk Access"), "{words}");
                assert!(words.contains("System Settings"), "{words}");
                assert!(words.to_lowercase().contains("skip"), "{words}");
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn other_failures_keep_their_own_kind_and_words() {
    let path = std::path::Path::new("/tmp/nothing-here");
    assert!(matches!(classify_io_error(path, &io::Error::from(io::ErrorKind::NotFound)), ImportError::NotFound { .. }));
    match classify_io_error(path, &io::Error::new(io::ErrorKind::InvalidData, "a broken file")) {
        ImportError::Unreadable { words, .. } => assert!(words.contains("a broken file"), "{words}"),
        other => panic!("{other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn a_mail_folder_macos_refuses_to_list_gives_needs_full_disk_access_and_no_panic() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let mail = home.path().join("Library").join("Mail");
    std::fs::create_dir_all(mail.join("V10")).unwrap();
    std::fs::set_permissions(&mail, std::fs::Permissions::from_mode(0o000)).unwrap();
    let restore = || std::fs::set_permissions(&mail, std::fs::Permissions::from_mode(0o755)).unwrap();
    if std::fs::read_dir(&mail).is_ok() {
        // a user that ignores folder permissions (root) cannot be refused; the unit tests above still prove the mapping
        restore();
        eprintln!("skipped: this user can list a folder with no permissions");
        return;
    }
    let found = find_apple_mail(home.path());
    let accounts: Vec<AccountAddress> = vec![];
    let imported = import(&Source::AppleMail { mail_dir: mail.clone() }, &ImportContext { accounts: &accounts, imported_account_id: IMPORTED }, &mut VecSink::default(), &mut |_| {});
    restore();
    assert!(matches!(found, Err(ImportError::NeedsFullDiskAccess { .. })), "{found:?}");
    match imported {
        Err(ImportError::NeedsFullDiskAccess { words, .. }) => assert!(words.contains("Full Disk Access")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_file_that_is_not_there_is_not_found_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let accounts: Vec<AccountAddress> = vec![];
    let err = import(&Source::Mbox { file: dir.path().join("missing") }, &ImportContext { accounts: &accounts, imported_account_id: IMPORTED }, &mut VecSink::default(), &mut |_| {}).unwrap_err();
    assert!(matches!(err, ImportError::NotFound { .. }), "{err:?}");
    assert!(err.words().contains("missing"), "{}", err.words());
}
