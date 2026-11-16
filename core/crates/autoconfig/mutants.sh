#!/bin/bash
# Each mutant breaks one property; the suite must go red on it. The file is restored from git after each run.
# Usage: core/crates/autoconfig/mutants.sh  (from anywhere; writes _captures/core-autoconfig/mutants.txt)
set -u
CRATE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$CRATE/../../.." && pwd)"
LOG="$ROOT/_captures/core-autoconfig/mutants.txt"
export PATH="$HOME/.cargo/bin:$PATH" CARGO_INCREMENTAL=0
: > "$LOG"
red=0; total=0
mutant() { # name file from to
  local name="$1" file="$CRATE/$2" from="$3" to="$4"
  # ONLY=<name> runs that one mutant (a targeted re-run after a test fix)
  if [ -n "${ONLY:-}" ] && [ "$ONLY" != "$name" ]; then return; fi
  total=$((total+1))
  if ! grep -qF -- "$from" "$file"; then echo "MUTANT $name: NOT APPLIED (pattern absent in $2)" | tee -a "$LOG"; return; fi
  until [ "$(LC_ALL=C sysctl -n vm.loadavg | awk '{printf "%d", $2}')" -lt 40 ]; do sleep 20; done
  FROM="$from" TO="$to" perl -0pi -e 's/\Q$ENV{FROM}\E/$ENV{TO}/' "$file"
  if (cd "$ROOT" && cargo test --manifest-path core/crates/autoconfig/Cargo.toml --offline -j 1 > "$LOG.run" 2>&1); then
    echo "MUTANT $name: GREEN (survived) -- the suite does not catch it" | tee -a "$LOG"
  else
    red=$((red+1))
    echo "MUTANT $name: RED, failing: $(grep -E '^test .* FAILED$|^error' "$LOG.run" | sed 's/^test //; s/ \.\.\. FAILED//' | tr '\n' ' ')" | tee -a "$LOG"
  fi
  git -C "$ROOT" checkout -- "core/crates/autoconfig/$2"
}
mutant step-order-swapped src/usecases/detect_provider.rs "[Step::Ispdb, Step::AutoconfigUrls," "[Step::AutoconfigUrls, Step::Ispdb,"
mutant guess-before-srv src/usecases/detect_provider.rs "Step::Srv, Step::Autodiscover, Step::MailRecord, Step::Guess]" "Step::Guess, Step::Srv, Step::Autodiscover, Step::MailRecord]"
mutant placeholder-unresolved src/parse/mozilla.rs '.replace("%EMAILLOCALPART%", email.local_part())' '.replace("%EMAILLOCALPART%", "%EMAILLOCALPART%")'
mutant starttls-labelled-ssl src/parse/mozilla.rs '"STARTTLS" => Some(Security::Starttls)' '"STARTTLS" => Some(Security::Ssl)'
mutant user-agent-dropped src/usecases/detect_provider.rs 'let ua = user_agent();' 'let ua = String::new();'
mutant imaps-port-off-by-one src/steps/guess.rs 'IMAPS_PORT, IMAP_PORT)' 'IMAPS_PORT + 1, IMAP_PORT)'
mutant srv-priority-inverted src/steps/srv.rs 'a.priority.cmp(&b.priority)' 'b.priority.cmp(&a.priority)'
mutant offline-not-refused src/steps/ispdb.rs '=> StepResult::Stop(Refusal::NoNetwork)' '=> StepResult::Missed'
mutant submissions-fallback-dropped src/steps/srv.rs '.or_else(|| best(dns, format!("_submissions._tcp.{d}")).map(|r| (r, Security::Ssl)))' ''
mutant nxdomain-read-as-existing src/adapters/dns_hickory.rs 'Err(e) if e.is_nx_domain() => Ok(DnsAnswer::Exists(false)),' 'Err(e) if e.is_nx_domain() => Ok(DnsAnswer::Exists(true)),'
mutant scoped-nameserver-kept src/adapters/dns_hickory.rs '.and_then(|a| a.parse().ok())' ".and_then(|a| a.split('%').next().and_then(|a| a.parse().ok()))"
mutant mx-step-dropped src/usecases/detect_provider.rs "Step::Autodiscover, Step::MailRecord, Step::Guess]" "Step::Autodiscover, Step::Guess, Step::Guess]"
mutant mx-after-guess src/usecases/detect_provider.rs "Step::Autodiscover, Step::MailRecord, Step::Guess]" "Step::Autodiscover, Step::Guess, Step::MailRecord]"
mutant mx-before-autodiscover src/usecases/detect_provider.rs "Step::Autodiscover, Step::MailRecord, Step::Guess]" "Step::MailRecord, Step::Autodiscover, Step::Guess]"
mutant m365-suffix-wrong src/steps/mail_record.rs 'mx_suffixes: &["mail.protection.outlook.com"]' 'mx_suffixes: &["protection.outlook.invalid"]'
mutant sign-in-needed-always-false src/usecases/detect_provider.rs 'let sign_in_needed = provider == Some(Provider::Microsoft365);' 'let sign_in_needed = false;'
mutant provider-not-read-from-host src/usecases/detect_provider.rs 'Outcome::Found(c) => provider_for_imap_host(&c.imap_host),' 'Outcome::Found(_) => None,'
mutant mx-preference-ignored src/steps/mail_record.rs 'records.sort_by_key(|r| r.preference);' 'records.sort_by_key(|r| std::cmp::Reverse(r.preference));'
mutant lookalike-suffix-accepted src/steps/mail_record.rs 'host.ends_with(&format!(".{suffix}"))' 'host.ends_with(suffix)'
mutant google-smtp-port-implicit src/steps/mail_record.rs 'smtp_host: "smtp.gmail.com", smtp_port: SUBMISSION_PORT' 'smtp_host: "smtp.gmail.com", smtp_port: SUBMISSION_PORT + 1'
mutant proton-imap-port-off src/steps/mail_record.rs 'imap_port: PROTON_BRIDGE_IMAP_PORT,' 'imap_port: PROTON_BRIDGE_IMAP_PORT + 1,'
mutant hickory-mx-preference-lost src/adapters/dns_hickory.rs 'MxRecord { preference: m.preference,' 'MxRecord { preference: 0,'
mutant cap-off-by-one src/adapters/body_cap.rs 'out.len() as u64 > limit' 'out.len() as u64 >= limit'
mutant cap-truncates-silently src/adapters/body_cap.rs 'reader.take(limit.saturating_add(1))' 'reader.take(limit)'
mutant cap-removed src/adapters/body_cap.rs 'if out.len() as u64 > limit {' 'if false {'
mutant cap-unbounded src/adapters/body_cap.rs 'reader.take(limit.saturating_add(1)).read_to_end' 'reader.take(u64::MAX).read_to_end'
mutant cap-constant-ignored src/adapters/body_cap.rs 'read_capped(reader, AUTOCONFIG_BODY_MAX_BYTES)' 'read_capped(reader, AUTOCONFIG_BODY_MAX_BYTES * 4)'
mutant yahoo-suffix-wrong src/steps/mail_record.rs 'mx_suffixes: &["am0.yahoodns.net"]' 'mx_suffixes: &["am0.yahoodns.invalid"]'
mutant yahoo-suffix-widened src/steps/mail_record.rs 'mx_suffixes: &["am0.yahoodns.net"]' 'mx_suffixes: &["yahoodns.net"]'
mutant yahoo-smtp-port-587 src/steps/mail_record.rs 'smtp_host: "smtp.mail.yahoo.com", smtp_port: SMTPS_PORT' 'smtp_host: "smtp.mail.yahoo.com", smtp_port: SUBMISSION_PORT'
mutant yahoo-imap-host-wrong src/steps/mail_record.rs 'imap_host: "imap.mail.yahoo.com"' 'imap_host: "imap.yahoo.com"'
mutant bare-domain-dropped src/steps/guess.rs 'format!("mail.{d}"), d.to_string()]' 'format!("mail.{d}")]'
mutant bare-domain-first src/steps/guess.rs 'vec![format!("imap.{d}"), format!("mail.{d}"), d.to_string()]' 'vec![d.to_string(), format!("imap.{d}"), format!("mail.{d}")]'
mutant bare-domain-in-smtp src/steps/guess.rs 'Protocol::Smtp => (vec![format!("smtp.{d}"), format!("mail.{d}")]' 'Protocol::Smtp => (vec![format!("smtp.{d}"), format!("mail.{d}"), d.to_string()]'
rm -f "$LOG.run"
echo "RESULT: $red of $total mutants red" | tee -a "$LOG"
[ "$red" -eq "$total" ]
