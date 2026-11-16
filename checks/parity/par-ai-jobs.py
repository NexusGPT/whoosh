#!/usr/bin/env python3
"""par-ai-jobs: the named features of the todo, each with one driven check on the fake provider.

    writing instructions · suggested replies · AI summaries · translate fallback through the provider
    (+ the draft in the owner's voice, dictation clean-up, sign-in code and needs-reply the lead named)

Each cell runs one test of core/crates/ai-jobs/tests/par_ai_jobs.rs and reads its verdict. The app-visible half (what the owner
sees on screen) is NOT judged here: no surface is locked-on-build, so no driven check can run against a running app
(docs/lanes/core-ai-jobs.md names that gap).
    python3 checks/parity/par-ai-jobs.py            run the cells
    python3 checks/parity/par-ai-jobs.py --mutants  also apply each mutant, demand the named cell goes red
Prints the cells, the mutants, then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import importlib.util
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("core_ai_jobs_check", os.path.join(os.path.dirname(HERE), "core-ai-jobs-check.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
SRC = base.SRC

CELLS = [
    ("writing-instructions", "par-ai-jobs", "writing instructions: the owner's standing instructions reach every writing job as their own words, and are absent when there are none", ["feature_writing_instructions"]),
    ("suggested-replies", "par-ai-jobs", "suggested replies: the asked number of replies, each with a label and a text; more than asked is refused", ["feature_suggested_replies"]),
    ("ai-summaries", "par-ai-jobs", "AI summaries: a thread summary with key points and open questions, every message sent oldest first; no key points is refused", ["feature_ai_summaries"]),
    ("translate-fallback", "par-ai-jobs", "translate through the provider: target language and text reach it, the answer carries the source language, the provider's refusal reaches the owner unchanged", ["feature_translate_fallback_through_the_provider"]),
    ("voice-dictation-codes", "par-ai-jobs", "draft in the owner's voice, dictation clean-up, sign-in code, needs-reply status", ["feature_draft_in_the_owners_voice_dictation_and_codes"]),
]

MUTANTS = [
    ("instructions-dropped", "writing-instructions", [(SRC + "prompt.rs", "let Some(v) = input.get(f.name) else { continue };", 'let Some(v) = input.get(f.name).filter(|_| f.name != "instructions") else { continue };')], "the standing instructions never reach the prompt"),
    ("reply-count-cap-ignored", "suggested-replies", [(SRC + "schema.rs", "if items.len() as u64 > cap {", "if false && items.len() as u64 > cap {")], "a model that sends more replies than asked is believed"),
    ("key-points-optional", "ai-summaries", [(SRC + "jobs/read.rs", 'req("key_points", list(', 'opt("key_points", list(')], "a summary without key points is accepted"),
    ("target-language-dropped", "translate-fallback", [(SRC + "prompt.rs", "let Some(v) = input.get(f.name) else { continue };", 'let Some(v) = input.get(f.name).filter(|_| f.name != "target_language") else { continue };')], "the provider is not told which language to translate into"),
    ("provider-refusal-replaced", "translate-fallback", [(SRC + "engine.rs", "let raw = self.provider.execute(self.key.as_ref(), &request)?;", 'let raw = self.provider.execute(self.key.as_ref(), &request).map_err(|_| refused(spec, "the provider failed"))?;')], "the owner reads a generic line instead of the provider's words"),
    ("dictation-field-renamed", "voice-dictation-codes", [(SRC + "jobs/write.rs", 'obj(vec![req("text", text("the cleaned text"))])', 'obj(vec![req("cleaned", text("the cleaned text"))])')], "the dictation job declares another field than its answer carries"),
]

if __name__ == "__main__":
    sys.exit(base.main(CELLS, MUTANTS))
