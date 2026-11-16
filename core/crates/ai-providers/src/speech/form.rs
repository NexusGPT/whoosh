//! A multipart form by hand: text fields and one file part. No extra crate for twenty lines.
use crate::ports::AudioClip;

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Returns (content type with boundary, body). The boundary is lengthened until it appears nowhere in the audio.
pub(crate) fn build(fields: &[(&str, &str)], file_field: &str, clip: &AudioClip) -> (String, Vec<u8>) {
    let mut boundary = String::from("whoosh-form-boundary");
    while contains(&clip.bytes, boundary.as_bytes()) {
        boundary.push('x');
    }
    let mut out = Vec::new();
    for (name, value) in fields {
        out.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    }
    let file_name = clip.file_name.replace(['"', '\r', '\n'], "_");
    out.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{file_field}\"; filename=\"{file_name}\"\r\nContent-Type: {}\r\n\r\n", clip.content_type).as_bytes(),
    );
    out.extend_from_slice(&clip.bytes);
    out.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), out)
}
