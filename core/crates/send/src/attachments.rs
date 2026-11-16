//! Contracts delta 22: the files a message carries. The message is `multipart/mixed`: the body, then one part per file, each base64,
//! its name in `Content-Disposition: attachment; filename*=` (RFC 2231, lettre's encoder) and in the `name=` of its `Content-Type` as an
//! RFC 2047 encoded word for the clients that read only that one. The bytes are base64-encoded here, in lines of 76 characters (RFC 2045),
//! from the bytes as read, so a file arrives byte for byte. The caps are the contract's, checked on the BUILT files.
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use contracts::{CoreError, ATTACHMENT_FILE_MAX_BYTES, ATTACHMENT_TOO_LARGE_CODE, ATTACHMENT_TOTAL_MAX_BYTES, ATTACHMENT_TOTAL_TOO_LARGE_CODE};
use lettre::message::header::{ContentDisposition, ContentTransferEncoding, ContentType};
use lettre::message::{Body, SinglePart};

/// One file as it goes: its name, its media type ("application/pdf") and its bytes as read when the message is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingFile {
    pub file_name: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// The line length of a base64 body (RFC 2045 6.8: at most 76 characters).
const BASE64_LINE: usize = 76;

fn refuse(code: &str, words: String) -> CoreError { CoreError::Invalid { field: code.into(), reason_words: words } }

/// The caps on the files as built: one file over ATTACHMENT_FILE_MAX_BYTES, or all together over ATTACHMENT_TOTAL_MAX_BYTES, is refused
/// in words with the contract's code (a file can grow between Add and Send; what goes is what is read now).
pub fn check_built(files: &[OutgoingFile]) -> Result<(), CoreError> {
    if let Some(f) = files.iter().find(|f| f.bytes.len() as u64 > ATTACHMENT_FILE_MAX_BYTES) {
        return Err(refuse(ATTACHMENT_TOO_LARGE_CODE, format!("\"{}\" is too large to send: remove it", f.file_name)));
    }
    let total: u64 = files.iter().map(|f| f.bytes.len() as u64).sum();
    if total > ATTACHMENT_TOTAL_MAX_BYTES {
        return Err(refuse(ATTACHMENT_TOTAL_TOO_LARGE_CODE, "the files together are too large to send: remove one".into()));
    }
    Ok(())
}

/// A name as an RFC 2047 encoded word (`=?utf-8?B?...?=`) when it holds anything but plain printable ASCII; else the name itself.
pub fn encoded_word(name: &str) -> String {
    if name.chars().all(|c| c.is_ascii_graphic() || c == ' ') && !name.contains('"') && !name.contains('\\') { return name.to_string(); }
    format!("=?utf-8?B?{}?=", STANDARD.encode(name.as_bytes()))
}

/// Base64 in lines of BASE64_LINE characters, CRLF between them.
pub fn base64_lines(bytes: &[u8]) -> String {
    let flat = STANDARD.encode(bytes);
    let mut out = String::with_capacity(flat.len() + flat.len() / BASE64_LINE * 2 + 2);
    for (i, chunk) in flat.as_bytes().chunks(BASE64_LINE).enumerate() {
        if i > 0 { out.push_str("\r\n"); }
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
    }
    out.push_str("\r\n");
    out
}

/// One file's part.
pub fn part(file: &OutgoingFile) -> SinglePart {
    let media = if file.media_type.trim().is_empty() { "application/octet-stream" } else { file.media_type.trim() };
    // fix-realcore round 7 (main100 --slice-attachsend): a name macOS handed decomposed ("Te\u{301}le\u{301}fonica") goes composed, as other mail
    // programs expect; they show a decomposed name with loose accents
    let name = parse::nfc(&file.file_name);
    let content_type = ContentType::parse(&format!("{media}; name=\"{}\"", encoded_word(&name)))
        .or_else(|_| ContentType::parse(&format!("application/octet-stream; name=\"{}\"", encoded_word(&name))))
        .unwrap_or(ContentType::TEXT_PLAIN);
    SinglePart::builder()
        .header(content_type)
        .header(ContentDisposition::attachment(&name))
        .header(ContentTransferEncoding::Base64)
        .body(Body::dangerous_pre_encoded(base64_lines(&file.bytes).into_bytes(), ContentTransferEncoding::Base64))
}

/// The bytes of one attachment of a raw message (a forwarded file not on this Mac, fetched with its message from the server): the `nth`
/// attachment named `file_name` (0 = the first of that name). None when the message holds no such part.
pub fn part_bytes(raw: &[u8], file_name: &str, nth: usize) -> Option<Vec<u8>> {
    let msg = mail_parser::MessageParser::new().parse(raw)?;
    // fix-realcore round 7: one name, two spellings (composed, decomposed) are the same file
    let want = parse::nfc(file_name);
    let bytes = msg.attachments().filter(|p| p.attachment_name().map(parse::nfc).as_deref() == Some(want.as_str())).nth(nth).map(|p| p.decoded().into_owned());
    bytes
}
