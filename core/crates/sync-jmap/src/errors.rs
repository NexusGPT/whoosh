//! jmap-client errors -> the contract's `CoreError`. The words carry what the server really said
//! (its status line, its problem body, the transport's cause chain), never only a generic status.
use contracts::{ActionFailed, CoreError};
use jmap_client::core::error::MethodErrorType;
use jmap_client::Error;

pub(crate) fn failed(account_name: &str, reason_words: String, retryable: bool) -> CoreError {
    CoreError::Action { failure: ActionFailed { account_name: account_name.to_string(), reason_words, retryable, code: None } }
}

/// What a failed HTTP status means to a retry: the sign-in was refused (never retry with the same
/// secret), the server is busy or broken (retry later), or the request itself is wrong.
fn status_retryable(status_line: &str) -> bool {
    let code: u16 = status_line.split_whitespace().next().and_then(|c| c.parse().ok()).unwrap_or(0);
    code == 408 || code == 429 || (500..600).contains(&code)
}

pub(crate) fn is_cannot_calculate_changes(e: &Error) -> bool {
    matches!(e, Error::Method(m) if m.p_type == MethodErrorType::CannotCalculateChanges)
}

pub(crate) fn is_auth_refused(e: &Error) -> bool {
    matches!(e, Error::Server(s) if s.starts_with("401") || s.starts_with("403"))
}

/// The cause chain of a transport error, for example "error sending request: connection refused".
fn chain(e: &dyn std::error::Error) -> String {
    let mut s = e.to_string();
    let mut cur = e.source();
    while let Some(c) = cur {
        s.push_str(": ");
        s.push_str(&c.to_string());
        cur = c.source();
    }
    s
}

pub(crate) fn map_error(account_name: &str, e: Error) -> CoreError {
    if is_auth_refused(&e) {
        let Error::Server(s) = &e else { unreachable!("is_auth_refused matched Server") };
        return failed(account_name, format!("the server refused the sign-in ({s})"), false);
    }
    match e {
        Error::Transport(t) => failed(account_name, format!("could not reach the server: {}", chain(&t)), true),
        Error::Server(s) => {
            let retry = status_retryable(&s);
            failed(account_name, format!("the server answered {s}"), retry)
        }
        Error::Parse(p) => failed(account_name, format!("the server's answer was not JMAP: {p}"), false),
        Error::Problem(p) => failed(account_name, format!("the server refused the request: {p}"), false),
        Error::Method(m) => {
            let retry = matches!(m.p_type, MethodErrorType::ServerUnavailable | MethodErrorType::ServerFail);
            failed(account_name, format!("the server could not do it: {m}"), retry)
        }
        other => failed(account_name, other.to_string(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_is_not_retryable_and_busy_is() {
        let a = map_error("Work", Error::Server("401 Unauthorized".into()));
        let b = map_error("Work", Error::Server("503 Service Unavailable".into()));
        let c = map_error("Work", Error::Server("404 Not Found".into()));
        let get = |e: &CoreError| match e {
            CoreError::Action { failure } => (failure.reason_words.clone(), failure.retryable, failure.account_name.clone()),
            _ => panic!("not an Action error"),
        };
        assert_eq!(get(&a).1, false);
        assert!(get(&a).0.contains("401 Unauthorized"), "{}", get(&a).0);
        assert_eq!(get(&b).1, true);
        assert_eq!(get(&c).1, false);
        assert_eq!(get(&a).2, "Work");
    }
}
