//! Wordings written independently of the fixture generator (the shapes public services use for
//! one-time codes, in English, French, Dutch and German) and wordings that must NOT be read as
//! a code. All senders and numbers are invented.
use parse::extract_login;

fn code(subject: &str, body: &str) -> Option<String> {
    extract_login(subject, body, None).code
}

#[test]
fn service_style_codes_are_found() {
    let cases: &[(&str, &str, &str)] = &[
        ("Verification code", "Your Example verification code is 123456. Don't share it with anyone.", "123456"),
        ("Security code", "Use 482913 as your account security code.", "482913"),
        ("", "482913 is your Example OTP. Do not share it with anyone.", "482913"),
        ("Your code", "Your Example ID Code is:\n\n739102\n\nEnter it on the sign-in screen.", "739102"),
        ("Confirmation", "Your confirmation code: 123-456. It will expire in 10 minutes.", "123456"),
        ("Sign in", "Enter the code below to log in.\n\n   941207 ", "941207"),
        ("Code de connexion", "Voici votre code de connexion : 530918. Il est valable 5 minutes.", "530918"),
        ("Verificatiecode", "Je verificatiecode is 664120. Niet delen met anderen.", "664120"),
        ("Anmeldung", "Ihr Einmal-Code lautet 205981. Bitte geben Sie ihn nicht weiter.", "205981"),
        ("Payment", "Your Lumen code 771204 confirms the payment of EUR 45.00. If this was not you, call us.", "771204"),
        ("Two-step", "Your two-step verification code is 318470", "318470"),
        ("Login", "Login code: 5920", "5920"),
    ];
    let mut wrong = Vec::new();
    for (s, b, want) in cases {
        let got = code(s, b);
        if got.as_deref() != Some(*want) {
            wrong.push(format!("{b:?}: want {want}, got {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn things_that_are_not_codes_are_left_alone() {
    let cases: &[(&str, &str)] = &[
        ("Your receipt", "Receipt 482913 for your purchase of 2 items, total EUR 45.00."),
        ("Meeting", "See you in room 4821 on 12 November 2026 at 15:00."),
        ("Shipping", "Your parcel 1Z999AA10123456784 is on its way. Postcode 9000 Gent."),
        ("Offer", "Use code SPRING25 for 25% off, valid until 30 November 2026."),
        ("Invoice", "Invoice 2026-1104 is due. Bank reference 123456789012."),
        ("Hello", "Call me on 0470 12 34 56 or 02 123 45 67 when you land."),
        ("Update", "We updated our code of conduct in 2025. Read the 2026 edition."),
        ("Security notice", "A new sign in to your account was seen at 14:32 from Brussels."),
        ("Quote", "The budget is 45000 euros and the deadline is 2026-12-01."),
    ];
    let mut wrong = Vec::new();
    for (s, b) in cases {
        if let Some(c) = code(s, b) {
            wrong.push(format!("{b:?}: found {c}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
