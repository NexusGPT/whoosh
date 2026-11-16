//! detect(email) -> DetectionResult: the five steps in the owner's order, stopping at the first that answers.
use crate::consts::PRODUCT_URL;
use crate::domain::{address::EmailAddress, config::{DetectionResult, Outcome, Provider, Refusal, Step}};
use crate::ports::{DnsPort, HttpPort, ProbePort};
use crate::steps::{self, mail_record::provider_for_imap_host, StepResult};

/// The result with its provider read from the configuration's IMAP host; a sign-in is needed for Microsoft 365.
fn finish(outcome: Outcome, steps_tried: Vec<Step>, microsoft_365_tenant: bool) -> DetectionResult {
    let provider = if microsoft_365_tenant {
        Some(Provider::Microsoft365)
    } else {
        match &outcome {
            Outcome::Found(c) => provider_for_imap_host(&c.imap_host),
            Outcome::ImapOnly(c) => provider_for_imap_host(&c.imap_host),
            Outcome::Refused(_) => None,
        }
    };
    let sign_in_needed = provider == Some(Provider::Microsoft365);
    DetectionResult { outcome, steps_tried, microsoft_365_tenant, provider, sign_in_needed }
}

/// The owner's order (docs/PRD.md "## Add account by IMAP with provider autodetection").
/// The mail record follows Autodiscover and precedes the guess (SPEC A76); the manual fields follow every step.
pub const STEP_ORDER: [Step; 6] = [Step::Ispdb, Step::AutoconfigUrls, Step::Srv, Step::Autodiscover, Step::MailRecord, Step::Guess];

/// "Whoosh/<version> (+https://whoosh.app)", sent on every autoconfig HTTP request.
pub fn user_agent() -> String { format!("Whoosh/{} (+{PRODUCT_URL})", env!("CARGO_PKG_VERSION")) }

pub struct Detector<H, D, P> {
    http: H,
    dns: D,
    probe: P,
}

impl<H: HttpPort, D: DnsPort, P: ProbePort> Detector<H, D, P> {
    pub fn new(http: H, dns: D, probe: P) -> Self { Self { http, dns, probe } }

    pub fn detect(&self, email: &str) -> DetectionResult { self.detect_with_progress(email, &mut |_| {}) }

    /// As `detect`, calling `on_step` before each step runs (the progress line of J4.1).
    pub fn detect_with_progress(&self, email: &str, on_step: &mut dyn FnMut(Step)) -> DetectionResult {
        let mut steps_tried = Vec::new();
        let email = match EmailAddress::parse(email) {
            Ok(e) => e,
            Err(reason) => return finish(Outcome::Refused(Refusal::InvalidAddress(reason)), steps_tried, false),
        };
        let ua = user_agent();
        for step in STEP_ORDER {
            on_step(step);
            steps_tried.push(step);
            let result = match step {
                Step::Ispdb => steps::ispdb::run(&self.http, &email, &ua),
                Step::AutoconfigUrls => steps::autoconfig_urls::run(&self.http, &email, &ua),
                Step::Srv => steps::srv::run(&self.dns, &email),
                Step::Autodiscover => steps::autodiscover::run(&self.http, &email, &ua),
                Step::MailRecord => steps::mail_record::run(&self.dns, &email),
                Step::Guess => steps::guess::run(&self.probe, &email),
            };
            match result {
                StepResult::Answered(outcome, m365) => return finish(outcome, steps_tried, m365),
                StepResult::Stop(refusal) => return finish(Outcome::Refused(refusal), steps_tried, false),
                StepResult::Missed => {}
            }
        }
        finish(Outcome::Refused(Refusal::NothingAnswered(email.domain().to_string())), steps_tried, false)
    }
}
