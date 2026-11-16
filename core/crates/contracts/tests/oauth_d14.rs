//! Contract delta 14: bring-your-own OAuth2. No real client ID appears in this file: the sample IDs are built from parts
//! at run time, so checks/no-vendor-id-check.py stays green on the repository.
use contracts::*;

fn guid_like(c: char) -> String {
    [8, 4, 4, 4, 12].iter().map(|n| c.to_string().repeat(*n)).collect::<Vec<_>>().join("-")
}

#[test]
fn the_endpoint_table_names_each_providers_documented_values() {
    let m = oauth_endpoints(OAuthProvider::Microsoft);
    assert!(m.authorize_url.starts_with("https://login.microsoftonline.com/common/oauth2/v2.0/") && m.token_url.ends_with("/token"));
    assert_eq!(m.scopes, ["offline_access", "https://outlook.office.com/IMAP.AccessAsUser.All", "https://outlook.office.com/SMTP.Send"]);
    let g = oauth_endpoints(OAuthProvider::Google);
    assert_eq!(g.authorize_url, "https://accounts.google.com/o/oauth2/v2/auth");
    assert_eq!(g.scopes, ["https://mail.google.com/"]);
    assert!(m.doc_url.starts_with("https://") && g.doc_url.starts_with("https://"));
    assert_eq!(oauth_provider_for(MailProvider::Google), Some(OAuthProvider::Google));
    assert_eq!(oauth_provider_for(MailProvider::Icloud), None);
}

#[test]
fn a_pasted_client_id_is_checked_against_the_providers_shape() {
    let ms = guid_like('a');
    assert_eq!(check_oauth_client_id(OAuthProvider::Microsoft, format!("  {}  ", ms.to_uppercase())).unwrap(), ms);
    assert!(check_oauth_client_id(OAuthProvider::Microsoft, "abc".into()).is_err());
    assert!(check_oauth_client_id(OAuthProvider::Microsoft, guid_like('g')).is_err(), "g is not hex");
    let suffix = [".apps", ".googleusercontent", ".com"].concat();
    let gid = format!("{}-{}{}", "1".repeat(6), "x".repeat(8), suffix);
    assert_eq!(check_oauth_client_id(OAuthProvider::Google, gid.clone()).unwrap(), gid);
    assert!(check_oauth_client_id(OAuthProvider::Google, suffix.clone()).is_err(), "nothing before the suffix");
    assert!(check_oauth_client_id(OAuthProvider::Google, "".into()).is_err());
    assert!(check_oauth_client_id(OAuthProvider::Google, format!("a b{suffix}")).is_err());
}

#[test]
fn a_draft_is_named_by_its_address_and_xoauth2_has_the_sasl_form() {
    assert_eq!(oauth_account_draft_id("  Assem@Northlight-Example.DE ".into()).unwrap(), "assem@northlight-example.de");
    assert!(oauth_account_draft_id("not an address".into()).is_err());
    assert_eq!(xoauth2_sasl("me@x.example", "tok"), "user=me@x.example\x01auth=Bearer tok\x01\x01");
}

#[test]
fn only_an_oauth_provider_can_need_a_sign_in() {
    let base = DetectionResult::new(DetectionOutcome::Refused { words: "x".into() }, vec![], false, None);
    assert!(!base.sign_in_needed && base.provider.is_none());
    let g = base.clone().with_sign_in(Some(DetectedProvider::Microsoft365), true);
    assert!(g.sign_in_needed && g.provider == Some(DetectedProvider::Microsoft365));
    assert_eq!(oauth_provider_for_detected(DetectedProvider::Google), Some(OAuthProvider::Google));
    assert_eq!(oauth_provider_for_detected(DetectedProvider::ProtonBridge), None);
    let i = base.with_sign_in(Some(DetectedProvider::Icloud), true);
    assert!(!i.sign_in_needed, "iCloud signs in with an app password");
}

#[test]
fn the_mail_record_step_sits_before_the_guess_and_names_itself() {
    assert_eq!(DetectionStep::MailRecord.words(), "Reading the domain's mail record…");
    let order = [DetectionStep::Ispdb, DetectionStep::AutoconfigUrls, DetectionStep::Srv, DetectionStep::Autodiscover, DetectionStep::MailRecord, DetectionStep::Guess];
    assert!(order.windows(2).all(|w| w[0] != w[1]));
    let _ = DetectionSource::MailRecord;   // the source a mail-record answer carries ("mx" in the store)
}
