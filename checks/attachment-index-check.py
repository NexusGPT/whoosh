#!/usr/bin/env python3
"""Attachment index check: J2.8 (an attachment found by what it contains), lane attachment-index.

Runs core/crates/attachment-index/examples/j2_corpus.rs over the J2 corpus (fixtures/tests-j2.json, the `attachment_content` map):
it indexes the three attachments with the fixture twins, files them in the REAL attachment_content table (the store's migrations
0001 to 0004), searches the way the one search does, and reads each hit's matched_in, attachment name and note. Two cells run the
crate's own cargo tests (PDF text, video sampling). The model is the twin (a picture is placed by its file-name words and its
reading): this proves the wiring and the J2.8 failure-path words, never retrieval quality; the real model is in
_captures/attachment-index/timings.txt.
    python3 checks/attachment-index-check.py             run the cells
    python3 checks/attachment-index-check.py --mutants   also apply each mutant, run, demand the named cell goes red
    ... --mutants --only id,id
Prints one line per cell, the mutants, then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
Needs cargo (offline, -j 1, CARGO_INCREMENTAL=0). A mutant edits a source file; the file is restored byte for byte.
"""
import json
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CORE = os.path.join(ROOT, "core")
SRC = "core/crates/attachment-index/src/"
CARGO = os.path.expanduser("~/.cargo/bin/cargo")
ENV = dict(os.environ, CARGO_INCREMENTAL="0")
QUERIES = ["the warehouse photo", "forklift pallets in the aisle", "scan notes", "site plan", "tax return", "the of in"]
NOT_DOWNLOADED = "not indexed yet, download to search inside"


def corpus_spec():
    d = json.load(open(os.path.join(ROOT, "fixtures", "tests-j2.json")))
    mime = {"image": "image/jpeg", "pdf": "application/pdf", "cad": "image/vnd.dwg"}
    rows = [(("att-" + f), v["message"], f, mime[v["kind"]], v["state"], v["text"]) for f, v in d["attachment_content"].items() if f != "note"]
    path = os.path.join(tempfile.gettempdir(), "attachment-index-j2spec-%d.tsv" % os.getpid())
    open(path, "w").write("\n".join("\t".join(r) for r in rows) + "\n")
    return path, d["expect"]


def run_corpus(spec):
    p = subprocess.run([CARGO, "run", "-q", "-p", "attachment-index", "--example", "j2_corpus", "--offline", "-j", "1", "--", spec, *QUERIES],
                       cwd=CORE, capture_output=True, text=True, env=ENV)
    if p.returncode != 0:
        return None, p.stdout + p.stderr
    out = {}
    for line in p.stdout.splitlines():
        if line.startswith("{"):
            j = json.loads(line)
            out[j["query"]] = j
    return out, p.stdout + p.stderr


def cargo_test(test):
    p = subprocess.run([CARGO, "test", "--offline", "-j", "1", "-p", "attachment-index", "--test", test], cwd=CORE, capture_output=True, text=True, env=ENV)
    return p.returncode == 0, p.stdout + p.stderr


def first(hits):
    return hits[0] if hits else {}


def cells(spec, exp):
    """Return [(id, ok, detail)]."""
    res, raw = run_corpus(spec)
    out = []
    if res is None:
        return [(c, None, "the corpus run did not build or run: " + raw[-300:]) for c in CELL_IDS]
    h = lambda q, k="hits": first(res.get(q, {}).get(k, []))
    both = lambda m: (m.get("message"), m.get("matched_in"), m.get("attachment"))
    want = (exp["photo_message"], "attachment-content", exp["photo_attachment"])
    meaning = h(exp["photo_query"], "meaning")
    out.append(("photo-by-what-it-shows", both(meaning) == want and both(h(exp["photo_query"])) == want, str(meaning)))
    words = h(exp["photo_content_query"], "words")
    out.append(("photo-by-words-inside-it", (words.get("message"), words.get("matched_in"), words.get("attachment")) == (exp["photo_message"], "attachment-content", exp["photo_attachment"]), str(words)))
    nd = h(exp["not_downloaded_query"])
    out.append(("not-downloaded-says-so", (nd.get("message"), nd.get("matched_in"), nd.get("attachment"), nd.get("note")) == (exp["not_downloaded_message"], "attachment-name", exp["not_downloaded_attachment"], NOT_DOWNLOADED), str(nd)))
    un = h(exp["unreadable_query"])
    out.append(("unreadable-by-name-and-says-so", (un.get("message"), un.get("matched_in"), un.get("attachment"), exp["unreadable_note_words"] in un.get("note", "")) == (exp["unreadable_message"], "attachment-name", exp["unreadable_attachment"], True), str(un)))
    # a query of filler words matches no word and no name (the twin model puts "the" in a vector, so only these two lists are read)
    none = [res.get("tax return", {}).get("hits"), res.get("the of in", {}).get("words"), res.get("the of in", {}).get("names")]
    out.append(("no-invented-hit", none == [[], [], []], str(none)))
    ok, raw = cargo_test("pdf_text")
    out.append(("pdf-text", ok, raw[-300:] if not ok else "pdf_text green"))
    ok, raw = cargo_test("plan")
    out.append(("video-sampling", ok, raw[-300:] if not ok else "plan green"))
    for cell, test in (("indexer-tests", "indexer"), ("hit-tests", "hits"), ("row-tests", "rows")):
        ok, raw = cargo_test(test)
        out.append((cell, ok, raw[-300:] if not ok else test + " green"))
    return out


CELL_IDS = ["photo-by-what-it-shows", "photo-by-words-inside-it", "not-downloaded-says-so", "unreadable-by-name-and-says-so", "no-invented-hit", "pdf-text", "video-sampling", "indexer-tests", "hit-tests", "row-tests"]

# (id, cell that must go red, [(file, old, new)], what the mutant breaks)
MUTANTS = [
    ("content-dropped", "photo-by-words-inside-it", [(SRC + "entry.rs", "parts.extend(self.ocr_text.as_deref());", "")], "the OCR text no longer joins the words of the attachment"),
    ("vectors-empty", "photo-by-what-it-shows", [(SRC + "indexer.rs", "e.vectors.push(EntryVector { key: format!(\"{}{}\", e.base_key(), req.key_suffix()), vector })", "e.vectors.push(EntryVector { key: format!(\"{}{}\", e.base_key(), req.key_suffix()), vector: { let _ = vector; Vec::new() } })")], "every media vector is stored empty"),
    ("not-downloaded-silent", "not-downloaded-says-so", [(SRC + "hits.rs", "IndexState::NotDownloaded => NOT_DOWNLOADED_WORDS.to_string(),", "IndexState::NotDownloaded => r.file_name.clone(),")], "a file not downloaded shows no note"),
    ("unreadable-dropped", "unreadable-by-name-and-says-so", [(SRC + "hits.rs", "q.iter().all(|w| name.iter().any(|n| n.starts_with(w.as_str())))", "r.state != IndexState::Unreadable && q.iter().all(|w| name.iter().any(|n| n.starts_with(w.as_str())))")], "an unreadable kind is left out of the name search"),
    ("name-beats-content", "photo-by-what-it-shows", [(SRC + "hits.rs", "if (content(&h.matched_in) && !content(&o.matched_in)) ||", "if (!content(&h.matched_in) && content(&o.matched_in)) ||")], "the name hit replaces the content hit for the same file"),
    ("filler-matches-all", "no-invented-hit", [(SRC + "hits.rs", "let Some(first) = q.first() else { return Vec::new() };", "let first = q.first().cloned().unwrap_or_default(); let first = &first;")], "a query with only filler words matches every file with text"),
    ("flate-not-decoded", "pdf-text", [(SRC + "pdf.rs", "ZlibDecoder::new(raw).read_to_end(&mut out).ok()?;", "let _ = ZlibDecoder::new(raw); return None;")], "a compressed PDF gives no text"),
    ("frame-cap-ignored", "video-sampling", [(SRC + "plan.rs", "let count = wanted.min(VIDEO_FRAMES_MAX);", "let count = wanted;")], "a long video gives a frame every 10 s without a cap"),
    ("unreadable-read-anyway", "indexer-tests", [(SRC + "indexer.rs", "if kind == AttachmentKind::Unreadable {", "if false {")], "a kind that cannot be read is read anyway"),
    ("refusal-unreported", "indexer-tests", [(SRC + "indexer_media.rs", "Ok(TranscriptOutcome::Refused { reason_words }) => e.notes.push(reason_words),", "Ok(TranscriptOutcome::Refused { .. }) => {}")], "a refused speech permission is not reported"),
    ("video-sound-skipped", "indexer-tests", [(SRC + "indexer_media.rs", "Ok(true) => {", "Ok(true) => { if true { continue; }")], "the sound track of a video is ignored"),
    ("zero-vector-accepted", "indexer-tests", [(SRC + "indexer.rs", "|| v.iter().all(|x| *x == 0.0)", "")], "a model answer of all zeros is stored"),
    ("kind-by-extension-lost", "video-sampling", [(SRC + "kind.rs", "UNREADABLE_EXT.contains(&ext) ||", "false ||")], "a .psd, whose MIME type says image, is read as a picture"),
    ("words-not-filed", "row-tests", [(SRC + "rows.rs", "SET extracted_text = ?2 WHERE id = ?1\"", "SET extracted_text = ?2 WHERE id = ?1 AND 0\"")], "the words never reach attachment.extracted_text, so the text index cannot see them"),
    ("stale-row-kept", "row-tests", [(SRC + "rows.rs", "DELETE FROM attachment_content WHERE attachment_id = ?1\"", "DELETE FROM attachment_content WHERE attachment_id = ?1 AND 0\"")], "an attachment that is no longer indexed keeps its old content"),
    ("not-downloaded-unseen", "row-tests", [(SRC + "rows.rs", "} else if local.is_none() {", "} else if false {")], "a file not downloaded reads as downloaded"),
    ("fewer-vectors-trusted", "indexer-tests", [(SRC + "indexer_media.rs", "if vs.len() != windows.len() {", "if false {")], "the model answers for fewer passages and the rest are dropped silently"),
]


def main():
    want_mutants = "--mutants" in sys.argv
    only = None
    if "--only" in sys.argv:
        only = set(sys.argv[sys.argv.index("--only") + 1].split(","))
    spec, exp = corpus_spec()
    checks = failed = unjudged = 0
    for cid, ok, detail in cells(spec, exp):
        checks += 1
        if ok is None:
            unjudged += 1
            print("NOT JUDGED  %-34s %s" % (cid, detail))
        elif ok:
            print("ok          %-34s" % cid)
        else:
            failed += 1
            print("FAILED      %-34s %s" % (cid, detail))
    if want_mutants:
        for mid, cell, edits, why in MUTANTS:
            if only and mid not in only:
                continue
            checks += 1
            saved = {}
            try:
                for f, old, new in edits:
                    path = os.path.join(ROOT, f)
                    saved.setdefault(f, open(path, "rb").read())
                    text = open(path).read()
                    if old not in text:
                        raise RuntimeError("mutant text not found in %s: %s" % (f, old[:60]))
                    open(path, "w").write(text.replace(old, new))
                got = {c: ok for c, ok, _ in cells(spec, exp)}
            except RuntimeError as e:
                unjudged += 1
                print("NOT JUDGED  mutant %-27s %s" % (mid, e))
                continue
            finally:
                for f, b in saved.items():
                    open(os.path.join(ROOT, f), "wb").write(b)
            if got.get(cell) is False:
                print("ok          mutant %-27s turns %s red (%s)" % (mid, cell, why))
            else:
                failed += 1
                print("FAILED      mutant %-27s %s stayed %s (%s)" % (mid, cell, got.get(cell), why))
    try:
        os.remove(spec)
    except OSError:
        pass
    print("%d checks, %d failed, %d not judged" % (checks, failed, unjudged))
    sys.exit(1 if failed or unjudged else 0)


main()
