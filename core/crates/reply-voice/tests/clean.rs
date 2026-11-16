//! Quote and signature stripping on five real-world shapes, and the cut to a size.
use reply_voice::{clean_sent_text, cut_to_chars};

#[test]
fn shape_1_angle_bracket_quotes_a_closing_run_goes() {
    let raw = "Sounds good, see you then.\n\n> Can we meet Tuesday at 10?\n> Claire\n";
    assert_eq!(clean_sent_text(raw), "Sounds good, see you then.");
}

#[test]
fn shape_1b_angle_bracket_quotes_inline_answers_keep_the_owner_lines() {
    let raw = "> Can you send the file?\nSent it this morning.\n> And the price?\nForty each.\n";
    assert_eq!(clean_sent_text(raw), "Sent it this morning.\nForty each.");
}

#[test]
fn shape_2_on_date_wrote_even_when_the_attribution_wraps() {
    let raw = "Thanks Claire, I will send it Friday.\n\nOn Mon, Nov 3, 2026 at 10:14 AM Claire Dupont\n<claire@example.com> wrote:\n> Hi, could you send the file?\n> Claire\n";
    assert_eq!(clean_sent_text(raw), "Thanks Claire, I will send it Friday.");
}

#[test]
fn shape_3_le_date_a_ecrit() {
    let raw = "Merci Claire, je vous l'envoie vendredi.\n\nLe lun. 3 nov. 2026 à 10:14, Claire Dupont <claire@example.com> a écrit :\n> Bonjour, pourriez-vous m'envoyer le fichier ?\n";
    assert_eq!(clean_sent_text(raw), "Merci Claire, je vous l'envoie vendredi.");
}

#[test]
fn shape_4_signature_delimiter_with_and_without_the_trailing_space() {
    assert_eq!(clean_sent_text("Cheers\nAssem\n-- \nAssem C.\nFounder, Whoosh\n+32 000 00 00 00\n"), "Cheers\nAssem");
    assert_eq!(clean_sent_text("Cheers\nAssem\n--\nAssem C.\nFounder\n"), "Cheers\nAssem");
}

#[test]
fn shape_5_outlook_header_block_english_and_french() {
    let en = "Yes, confirmed.\n\nFrom: Claire Dupont <claire@example.com>\nSent: Monday, November 3, 2026 10:14 AM\nTo: Assem\nSubject: Re: lunch\n\nCan we do Tuesday?\n";
    assert_eq!(clean_sent_text(en), "Yes, confirmed.");
    let fr = "Oui, c'est confirmé.\n\nDe : Claire Dupont <claire@example.com>\nEnvoyé : lundi 3 novembre 2026 10:14\nÀ : Assem\nObjet : RE: déjeuner\n\nOn fait mardi ?\n";
    assert_eq!(clean_sent_text(fr), "Oui, c'est confirmé.");
}

#[test]
fn a_signature_then_a_quote_both_go() {
    let raw = "Okay.\n-- \nAssem\n\nOn Mon, Nov 3, 2026 at 10:14 AM Claire <c@example.com> wrote:\n> hi\n";
    assert_eq!(clean_sent_text(raw), "Okay.");
}

#[test]
fn a_message_that_is_only_a_quote_leaves_nothing() {
    assert_eq!(clean_sent_text("On Mon, Nov 3, 2026 at 10:14 AM Claire <c@example.com> wrote:\n> hi\n"), "");
    assert_eq!(clean_sent_text("> hi\n> there\n"), "");
}

#[test]
fn a_message_that_is_only_an_outlook_header_block_leaves_nothing() {
    let raw = "From: Claire Dupont <claire@example.com>\nSent: Monday, November 3, 2026 10:14 AM\nTo: Assem\nSubject: Fwd: lunch\n\nCan we do Tuesday?\n";
    assert_eq!(clean_sent_text(raw), "");
}

#[test]
fn a_plain_message_comes_back_whole_and_crlf_is_read() {
    assert_eq!(clean_sent_text("Hello\r\nHow are you?\r\n"), "Hello\nHow are you?");
}

#[test]
fn a_dash_inside_a_line_is_not_a_signature() {
    assert_eq!(clean_sent_text("Ready -- see you at five.\nBye"), "Ready -- see you at five.\nBye");
}

#[test]
fn cut_to_chars_keeps_a_text_that_fits_and_cuts_at_a_word() {
    assert_eq!(cut_to_chars("abcde", 5), "abcde");
    assert_eq!(cut_to_chars("hello wonderful world", 14), "hello…");
    assert_eq!(cut_to_chars("hello wonderful world", 20), "hello wonderful…");
    assert!(cut_to_chars("hello wonderful world", 14).chars().count() <= 14);
}

#[test]
fn cut_to_chars_counts_characters_not_bytes_and_a_budget_of_zero_is_empty() {
    assert_eq!(cut_to_chars("éééé", 4), "éééé");
    assert_eq!(cut_to_chars("éééééé", 4), "ééé…");
    assert_eq!(cut_to_chars("anything", 0), "");
}
