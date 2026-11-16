use attachment_index::extract_pdf_text;

const PLAIN: &[u8] = include_bytes!("fixtures/plain.pdf");
const FLATE: &[u8] = include_bytes!("fixtures/flate.pdf");
const SCAN: &[u8] = include_bytes!("fixtures/scan.pdf");

#[test]
fn a_plain_pdf_gives_its_words_in_order() {
    let t = extract_pdf_text(PLAIN).expect("text");
    assert_eq!(t, "Quarterly brochure quote\nHeavy matte stock, 170 g, two colours\nUnit price 0.84 EUR for 12,000 copies");
}

#[test]
fn a_flate_compressed_pdf_gives_the_same_words() {
    assert!(String::from_utf8_lossy(FLATE).contains("FlateDecode"), "the fixture must really be compressed");
    assert!(FLATE.windows(8).all(|w| w != b"brochure"), "the words must not be readable in the file itself");
    assert_eq!(extract_pdf_text(FLATE), extract_pdf_text(PLAIN));
}

#[test]
fn a_gap_inside_a_text_array_is_a_space() {
    let t = extract_pdf_text(PLAIN).unwrap();
    assert!(t.contains("170 g, two colours"), "{t}");
}

#[test]
fn a_pdf_with_no_text_operators_gives_none_so_pdfkit_is_asked() {
    assert_eq!(extract_pdf_text(SCAN), None);
}

#[test]
fn bytes_that_are_not_a_pdf_and_encrypted_files_give_none() {
    assert_eq!(extract_pdf_text(b"hello"), None);
    let mut enc = PLAIN.to_vec();
    enc.extend_from_slice(b"\n/Encrypt 9 0 R\n");
    assert_eq!(extract_pdf_text(&enc), None);
}

#[test]
fn glyph_ids_that_are_not_language_are_declined() {
    let junk = b"%PDF-1.4\n1 0 obj\n<< /Length 40 >>\nstream\nBT <00010002000300040005> Tj ET\nendstream\nendobj\n%%EOF";
    assert_eq!(extract_pdf_text(junk), None);
}
