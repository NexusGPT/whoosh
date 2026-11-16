# Proof files: how each open item is proved on a build

One JSON file per open item of the task board: `checks/proofs/g<N>/<item id, 8 characters>.json`. The lead's runner reads every file, runs each
proof on the current build, runs each broken copy, and writes one result line per item. An item is closed only on its result line.

```json
{
  "item": "64f2e8a7",
  "full_id": "64f2e8a7-ebd8-4cb9-a8f6-4ca5586f911b",
  "title": "Match typed letters inside words in the engine",
  "state": "prove",
  "proofs": [
    {"kind": "cargo", "run": "--release -p engine --test engine_search_inside", "pass": "test result: ok\\. 3 passed; 0 failed", "fail": "FAILED|panicked", "timeout_s": 900},
    {"kind": "unit", "run": "theCountLineSaysHowManyMatch,aSubjectMatchInsideALongerWordIsBold", "pass": "passed", "fail": "failed"},
    {"kind": "hidden", "core": "fixture", "run": "python3 checks/parity/par-searchmac.py", "pass": "PASS count-line", "fail": "FAIL count-line", "timeout_s": 300},
    {"kind": "static", "run": "python3 checks/i18n-check.py --catalogue", "pass": "0 missing", "fail": "missing [1-9]", "timeout_s": 300},
    {"kind": "lead", "run": "search 'consultant' on a private copy of the owner's mail, counts only", "pass": "3 of 3 in the match group"}
  ],
  "mutants": [
    {"file": "core/crates/search-text/src/query.rs", "find": "while scanned < SUBSTRING_SCAN_MAX && all.advance() {", "replace": "while false && all.advance() {", "proof": 0, "red": "a_consultant_finds.*FAILED"}
  ],
  "gap": null,
  "owner": null,
  "notes": "the speed test needs the release build"
}
```

- `state`: `prove` (the code exists; the proofs below settle it) · `gap` (code, a test or a check case is missing: `gap` says what, in which
  files, and how large: S under an hour, M a few hours, L more) · `owner` (only the owner can end it: `owner` says what they do) ·
  `outside` (it needs an account, a device or a download this Mac does not have: `notes` says which).
- `proofs[].kind`:
  - `cargo`: `run` is what follows `cargo test`. The lead runs it; one compile at a time.
  - `unit`: `run` is a comma-separated list of Swift test function names that exist under `Tests/`.
  - `hidden`: `run` is a check command that drives the app's hidden test copy; `core` is `fixture` or `real`. The lead runs it through
    `scripts/launch-one.sh` with `WHOOSH_RIG_HIDDEN=1`. No window ever shows.
  - `static`: a command that launches no app and compiles nothing.
  - `lead`: needs the lead's own hands (a private copy of the owner's data read as counts only, the installed build, a quiet machine).
- `pass` is a regular expression the output must hold; `fail` one it must not hold. A check that prints nothing for a passing case cannot be
  proved: that is a `gap` of kind `check` ("print a line per passing case").
- `mutants[]`: one deliberate one-line break per behaviour the item names. `find` is exact and occurs once in `file`; `proof` is the index of
  the proof that must go red; `red` is a regular expression of the red line. The runner applies it, runs that proof, restores the file.
- Every file, test name and check case named in a proof file exists in the repository at the commit the file was written on, with its path
  and line in `notes`. A name that does not exist is a `gap`, never a proof.
