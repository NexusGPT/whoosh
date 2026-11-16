//! Times the pure-Rust PDF reader on the fixture files and on a large synthetic PDF (many pages). Debug or release as built.
use std::time::Instant;

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for name in ["plain.pdf", "flate.pdf"] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let t = Instant::now();
        let text = attachment_index::extract_pdf_text(&bytes);
        println!("{name}: {} bytes, {} chars in {:.3} ms", bytes.len(), text.map_or(0, |t| t.len()), t.elapsed().as_secs_f64() * 1000.0);
    }
    // 500 pages of the plain fixture's content stream: a long report
    let plain = std::fs::read(dir.join("plain.pdf")).unwrap();
    let mut big = b"%PDF-1.4\n".to_vec();
    let one = String::from_utf8_lossy(&plain).into_owned();
    let start = one.find("stream\n").unwrap() + 7;
    let end = one.find("\nendstream").unwrap();
    let page = one[start..end].to_string();
    for i in 0..500 {
        big.extend_from_slice(format!("{} 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n", i + 1, page.len(), page).as_bytes());
    }
    let t = Instant::now();
    let text = attachment_index::extract_pdf_text(&big);
    println!("synthetic 500-page PDF: {} bytes, {} chars in {:.3} ms", big.len(), text.map_or(0, |t| t.len()), t.elapsed().as_secs_f64() * 1000.0);
}
