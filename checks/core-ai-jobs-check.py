#!/usr/bin/env python3
"""AI jobs core check: fn-ai-jobs (docs/plan.json lane core-ai-jobs).

Runs `cargo test -p ai-jobs` (the fake provider, fixtures in core/crates/ai-jobs/fixtures/jobs.json) and reads each named test's
verdict. One cell is not a cargo test: "licence" loads checks/licence-check.py and asks it whether any file of this crate shares a
string of 40 characters or more with the AGPL boilerplate's prompt files (folder: env AGPL_BOILERPLATE_DIR or the licence check's own
default; absent = NOT JUDGED).
    python3 checks/core-ai-jobs-check.py            run the cells
    python3 checks/core-ai-jobs-check.py --mutants  also apply each mutant, run, demand the named cell goes red
    ... --mutants --only id,id  only those mutants
Prints one line per cell, the mutants, then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
Needs cargo (offline, -j 1). A mutant edits a source file, the check runs, and the file is restored byte for byte.
"""
import importlib.util
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CORE = os.path.join(ROOT, "core")
SRC = "core/crates/ai-jobs/src/"
CARGO = os.path.expanduser("~/.cargo/bin/cargo")
LICENCE = "@licence"

CELLS = [
    ("roster", "fn-ai-jobs", "45 jobs, each with one spec and one fixture, no job without either", ["every_job_has_one_spec_and_one_fixture"]),
    ("schema-per-job", "fn-ai-jobs", "with the fake provider each job returns exactly its declared schema", ["each_job_returns_its_schema_on_the_fake"]),
    ("malformed-refused", "fn-ai-jobs", "a malformed answer is refused in words naming the job; the model's own text never reaches the owner", ["a_malformed_answer_is_refused_in_words_and_never_surfaced_raw", "an_answer_in_a_code_fence_or_after_a_line_of_talk_is_accepted_and_nothing_else_is_kept", "a_pick_outside_its_values_is_refused_and_a_pick_in_other_case_is_normalised"]),
    ("not-made-up", "fn-ai-jobs", "an answer naming something not offered, or a code not in the message, is refused", ["an_answer_naming_something_that_was_not_offered_is_refused", "a_sign_in_code_that_is_not_in_the_message_is_refused_as_made_up"]),
    ("input-checked", "fn-ai-jobs", "bad input is refused before any request leaves", ["input_is_checked_before_the_provider_is_called"]),
    ("provider-words", "fn-ai-jobs", "the provider's refusal reaches the owner unchanged; the key travels as a reference and is not in the prompt", ["a_providers_refusal_passes_through_unchanged", "the_key_reference_and_the_callers_ceiling_reach_the_provider_and_no_secret_is_in_the_words"]),
    ("data-not-orders", "fn-ai-jobs", "mail cannot close its data block; owner words and mail are marked apart; the prompt is made from the schema", ["mail_cannot_close_its_data_block_or_pose_as_the_owner", "owner_words_and_mail_are_marked_apart_and_the_prompt_is_made_from_the_schema"]),
    ("voice-sent-only", "fn-ai-jobs", "the voice profile job reads only the owner's own Sent mail", ["only_the_owners_own_sent_mail_reaches_the_model"]),
    ("voice-weekly", "fn-ai-jobs", "voice_profile is rewritten once per VOICE_REFRESH_DAYS (the ledger row), kept on a refused answer, untouched without sent mail", ["the_refresh_window_is_the_ledger_row", "the_profile_is_saved_and_rewritten_once_per_window", "a_refused_answer_keeps_the_old_profile", "without_sent_mail_nothing_is_asked_and_nothing_is_written"]),
    ("voice-in-draft", "fn-ai-jobs", "the saved profile reaches the draft job as the owner's voice", ["the_saved_profile_reaches_the_draft_in_the_owners_voice"]),
    ("licence", "fn-ai-jobs", "no string of 40 characters or more in this crate is shared with the boilerplate's prompt files", [LICENCE]),
]

# (id, cell, [(file, old text, new text), ...], why it must turn the cell red). Each `old` is replaced everywhere it occurs.
MUTANTS = [
    ("missing-field-accepted", "malformed-refused", [(SRC + "schema.rs", "None | Some(Value::Null) if f.required => return", "None | Some(Value::Null) if f.required && false => return")], "an answer with a required field missing is accepted"),
    ("job-returns-wrong-schema", "schema-per-job", [(SRC + "jobs/read.rs", 'req("key_points", list(', 'req("points", list(')], "the summary job declares another field name than the answer carries: its answer no longer fits"),
    ("raw-text-in-refusal", "malformed-refused", [(SRC + "engine.rs", ".map_err(|why| refused(spec, why))?;", ".map_err(|_| refused(spec, &raw))?;")], "the owner reads the model's own text"),
    ("mail-closes-its-block", "data-not-orders", [(SRC + "prompt.rs", "if c == '<' && (", "if false && c == '<' && (")], "a </mail> inside a message ends the data block"),
    ("voice-window-ignored", "voice-weekly", [(SRC + "voice.rs", "if age_ms < self.window_ms {", "if false && age_ms < self.window_ms {")], "the profile is rebuilt on every call"),
    ("voice-reads-other-senders", "voice-sent-only", [(SRC + "voice.rs", ".filter(|m| m.from_address.trim().eq_ignore_ascii_case(address.trim()) && !m.text.trim().is_empty())", ".filter(|m| !m.text.trim().is_empty())")], "mail from others reaches the model"),
    ("offered-list-not-checked", "not-made-up", [(SRC + "engine.rs", "if values.iter().any(|v| !allowed.iter().any(|a| listed(a, v))) {", "if false {")], "an unoffered slot, category or file is accepted"),
    ("made-up-code-accepted", "not-made-up", [(SRC + "engine.rs", "if values.iter().any(|v| !source.contains(&plain(v))) {", "if false {")], "a code that is not in the message is accepted"),
    ("input-not-checked", "input-checked", [(SRC + "engine.rs", "input::check(spec, input)?;", "")], "bad input reaches the provider"),
    ("provider-words-replaced", "provider-words", [(SRC + "engine.rs", "let raw = self.provider.execute(self.key.as_ref(), &request)?;", 'let raw = self.provider.execute(self.key.as_ref(), &request).map_err(|_| refused(spec, "the provider failed"))?;')], "the provider's own words are replaced by a generic line"),
    ("voice-field-dropped", "voice-in-draft", [(SRC + "prompt.rs", "let Some(v) = input.get(f.name) else { continue };", 'let Some(v) = input.get(f.name).filter(|_| f.name != "voice") else { continue };')], "the owner's voice never reaches the prompt"),
    ("boilerplate-line-pasted", "licence", [(SRC + "jobs/write.rs", "//! Jobs that put words in the composer", "//! @BOILERPLATE_LINE@")], "a prompt line of the boilerplate is pasted into a source file"),
]


def cargo_test(extra=()):
    """Run the crate's tests; return ({test: 'ok'|'FAILED'}, build_failed, raw output)."""
    p = subprocess.run([CARGO, "test", "--offline", "-j", "1", "-p", "ai-jobs", "--no-fail-fast", *extra, "--", "--test-threads", "2"], cwd=CORE, capture_output=True, text=True)
    out = p.stdout + p.stderr
    results = {m.group(1): m.group(2) for m in re.finditer(r"^test (\S+) \.\.\. (ok|FAILED)", out, re.M)}
    return results, (not results and p.returncode != 0), out


def why_failed(out, name):
    m = re.search(r"---- %s stdout ----\n(.*?)(?:\n\n|\Z)" % re.escape(name), out, re.S)
    return " | ".join(l.strip() for l in (m.group(1).splitlines() if m else [])[:4])


def licence_module():
    spec = importlib.util.spec_from_file_location("licence_check", os.path.join(ROOT, "checks", "licence-check.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def boilerplate_line():
    """One long line from a boilerplate prompt file (the text the mutant pastes)."""
    lic = licence_module()
    for d, _, fs in os.walk(lic.BOILER):
        for f in sorted(fs):
            p = os.path.join(d, f)
            if f.endswith(".ts") and re.search(r"prompt|/ai/", p, re.I):
                for line in open(p, encoding="utf-8", errors="replace"):
                    t = line.strip().strip("`'\"")
                    if len(t) >= 70 and sum(c.isalpha() for c in t) > 50 and "//" not in t[:2]:
                        return t[:90].replace("\n", " ")
    return None


def licence_verdict():
    """('ok'|'FAILED'|'NOT JUDGED', words): every file under core/crates/ai-jobs against the boilerplate's 40-character windows."""
    lic = licence_module()
    if not os.path.isdir(lic.BOILER):
        return "NOT JUDGED", "boilerplate folder not found: %s" % lic.BOILER
    sh, nb = lic.boiler_shingles()
    if nb == 0 or not sh:
        return "NOT JUDGED", "no boilerplate prompt file found"
    # control: the matcher must find a boilerplate line pasted into text (else a green here proves nothing)
    line = boilerplate_line()
    if line is None or not lic.shared_with_boiler("x " + line + " y", sh):
        return "NOT JUDGED", "control failed: the matcher does not find a boilerplate line"
    hits, n = [], 0
    base = os.path.join(ROOT, "core", "crates", "ai-jobs")
    for d, ds, fs in os.walk(base):
        ds[:] = [x for x in ds if x != "target"]
        for f in fs:
            p = os.path.join(d, f)
            if f.endswith(lic.SCAN_EXT):
                n += 1
                if lic.shared_with_boiler(open(p, encoding="utf-8", errors="replace").read(), sh):
                    hits.append(os.path.relpath(p, ROOT))
    if n == 0:
        return "NOT JUDGED", "no file scanned"
    return ("FAILED", "shared with the boilerplate: %s" % ", ".join(hits)) if hits else ("ok", "%d files scanned, %d boilerplate windows" % (n, len(sh)))


def judge(results, tests):
    if tests == [LICENCE]:
        v, words = licence_verdict()
        return ([LICENCE] if v == "NOT JUDGED" else []), ([LICENCE] if v == "FAILED" else []), words
    missing = [t for t in tests if t not in results]
    failed = [t for t in tests if results.get(t) == "FAILED"]
    return missing, failed, ""


def run_cells(cells):
    results, build_failed, out = cargo_test()
    if any(v == "FAILED" for v in results.values()) or build_failed:
        os.makedirs(os.path.join(ROOT, "_captures", "core-ai-jobs"), exist_ok=True)
        open(os.path.join(ROOT, "_captures", "core-ai-jobs", "last-red.txt"), "w").write(out)
        print("raw cargo output kept in _captures/core-ai-jobs/last-red.txt")
    n = f = u = 0
    for cid, todo, what, tests in cells:
        n += 1
        if build_failed and tests != [LICENCE]:
            u += 1
            print("NOT JUDGED  %-18s %s (the crate did not build)" % (cid, todo))
            continue
        missing, failed, words = judge(results, tests)
        if missing:
            u += 1
            print("NOT JUDGED  %-18s %s: %s" % (cid, todo, words or "no verdict for " + ", ".join(missing)))
        elif failed:
            f += 1
            print("RED         %-18s %s: %s -- failing: %s %s" % (cid, todo, what, ", ".join(failed), words))
            for t in failed:
                if t != LICENCE:
                    print("            %s: %s" % (t, why_failed(out, t)))
        else:
            print("green       %-18s %s: %s%s" % (cid, todo, what, (" [" + words + "]") if words else ""))
    return n, f, u


def run_mutants(mutants, cells):
    """Apply each mutant, run, demand the named cell is red, restore the file byte for byte."""
    bad = 0
    tests_of = {c[0]: c[3] for c in cells}
    for mid, cell, edits, why in mutants:
        originals = {}
        for rel, old, new in edits:
            originals.setdefault(rel, open(os.path.join(ROOT, rel), "rb").read())
        if any(old not in originals[rel].decode() for rel, old, new in edits):
            print("MUTANT %-28s CANNOT APPLY: text not found" % mid)
            bad += 1
            continue
        pasted = boilerplate_line() if any("@BOILERPLATE_LINE@" in new for _, _, new in edits) else None
        if any("@BOILERPLATE_LINE@" in new for _, _, new in edits) and pasted is None:
            print("MUTANT %-28s CANNOT APPLY: no boilerplate line to paste" % mid)
            bad += 1
            continue
        try:
            texts = {rel: o.decode() for rel, o in originals.items()}
            for rel, old, new in edits:
                texts[rel] = texts[rel].replace(old, new.replace("@BOILERPLATE_LINE@", pasted or ""))
            for rel, t in texts.items():
                open(os.path.join(ROOT, rel), "wb").write(t.encode())
            if tests_of[cell] == [LICENCE]:
                results, build_failed, out = {}, False, ""
            else:
                results, build_failed, out = cargo_test()
            missing, failed, words = judge(results, tests_of[cell])
        finally:
            for rel, o in originals.items():
                open(os.path.join(ROOT, rel), "wb").write(o)
        assert all(open(os.path.join(ROOT, rel), "rb").read() == o for rel, o in originals.items())
        if build_failed:
            print("MUTANT %-28s DID NOT BUILD (a mutant must compile)" % mid)
            bad += 1
        elif failed:
            print("MUTANT %-28s red in %s: %s  [%s]" % (mid, cell, ", ".join(failed), why))
        else:
            print("MUTANT %-28s SURVIVED: cell %s stayed green  [%s]" % (mid, cell, why))
            bad += 1
    return bad


def main(cells=CELLS, mutants=MUTANTS):
    n, f, u = run_cells(cells)
    if "--mutants" in sys.argv:
        only = sys.argv[sys.argv.index("--only") + 1].split(",") if "--only" in sys.argv else None
        print("MUTANTS (each applied alone, run, file restored):")
        f += run_mutants([m for m in mutants if only is None or m[0] in only], cells)
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    return 1 if (f or u) else 0


if __name__ == "__main__":
    sys.exit(main())
