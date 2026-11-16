//! Who opens the provider's sign-in page. The app: the person's own browser (`open` on macOS). The hidden test copy: never
//! a browser (the owner: no window, ever); with the check's local authority it plays the browser itself over HTTP.
use std::sync::Arc;

use contracts::CoreError;

use super::authority::{host_of, AuthorityHttp};

/// The person's default browser, by `/usr/bin/open` (HTTPS pages only).
pub struct SystemBrowser;

impl oauth::OpenBrowser for SystemBrowser {
    fn open(&self, url: &str) -> Result<(), CoreError> {
        if !url.starts_with("https://") { return Err(CoreError::Invalid { field: "authorize_url".into(), reason_words: format!("the sign-in page at {} is not HTTPS, so it was not opened", host_of(url)) }); }
        #[cfg(target_os = "macos")]
        {
            let out = std::process::Command::new("/usr/bin/open").arg(url).output()
                .map_err(|e| CoreError::Unexpected { reason_words: format!("the browser could not be started: {e}") })?;
            if out.status.success() { return Ok(()); }
            let words = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(CoreError::Unexpected { reason_words: format!("the browser did not open ({}): {words}", out.status) })
        }
        #[cfg(not(target_os = "macos"))]
        { Err(CoreError::Unexpected { reason_words: "this build opens no browser on this system".into() }) }
    }
}

/// The hidden test copy without a check authority: it opens nothing and says so.
pub struct NoBrowser;

impl oauth::OpenBrowser for NoBrowser {
    fn open(&self, _url: &str) -> Result<(), CoreError> {
        Err(CoreError::Unexpected { reason_words: "the hidden test copy opens no browser".into() })
    }
}

/// The browser of the tests and the hidden check (clear HTTP to this Mac only): asks the local authority's sign-in page,
/// takes the redirect it answers and brings it to the loopback, on its own thread, as a person approving would.
pub struct Follower { pub http: Arc<AuthorityHttp> }

impl oauth::OpenBrowser for Follower {
    fn open(&self, url: &str) -> Result<(), CoreError> {
        if !super::authority::allowed(url) || url.starts_with("https://") {
            return Err(CoreError::Invalid { field: "authorize_url".into(), reason_words: "the test browser follows this Mac's own sign-in page only".into() });
        }
        let (http, url) = (self.http.clone(), url.to_string());
        std::thread::spawn(move || match http.get_location(&url) {
            Ok(Some(back)) => if let Err(e) = http.get_location(&back) { eprintln!("test browser: the loopback did not answer: {e}") },
            Ok(None) => eprintln!("test browser: the sign-in page at {} sent no redirect", host_of(&url)),
            Err(e) => eprintln!("test browser: the sign-in page did not answer: {e}"),
        });
        Ok(())
    }
}
