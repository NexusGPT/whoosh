# Journeys

`python3 journeys/run.py [J1 J2 ...] [--dry]` (Python 3.12, standard library only).

- Discovers `journeys/J*.py`, runs each over the control socket, prints one line `N checks, F failed, U not judged`, writes `docs/COVERAGE.md`, exits 1 unless every journey is GREEN.
- Per-journey lines and every failing check go to stderr.
- Verdicts: GREEN, RED (a check failed), COULD-NOT-DRIVE (no socket in `$WHOOSH_SOCKET_DIR/whoosh.sock`, or a read failed), TIMING-NOT-JUDGED (the app was driven and every other check passed; a timing over its budget was measured above PERF_LOAD_MAX, journeys/lib/loadgate.py; not green). A missing socket never crashes a journey.
- `--dry` forces dry mode: same code, no socket, every check not judged. N is the same dry or live.
- A run with a journey filter does not rewrite `docs/COVERAGE.md`.

## Writing a journey

One file per journey, `ID`, `TITLE`, `TARGET`, `VISIBLE_OUTCOME`, `drive(j)`, and a `# MUTANTS` comment block with one row per check: `# J1.2/row-leaves | the mutation that turns it red`. `run.py` reports a check with no row and a row with no check.

Per step: `s = j.step("J1.2", title)`, actions `s.key / s.type / s.click / s.raw`, `s.arrive(("key"|"click", control), surface)` for the arrival by named control, and checks `s.expect(name, thunk, expected)`, `expect_le`, `expect_in`, `expect_not_in`. The thunk reads through `s.ctl`. Expected values come from `lib/fixture.py` (the design fixture and `docs/CONSTANTS.md`), never from a built screen. Any absence check carries a positive control check (the same probe returning rows elsewhere).

## Wire contract (ASSUMPTION until docs/CONTROL-SOCKET.md lands)

The verbs and reply fields are the planned contract, all in `lib/ctl.py`. The accessibility ids, the `uistate` fields, and the `fixture`, `clock` and `core` verbs the journeys use are this lane's assumption; the root lane's `docs/CONTROL-SOCKET.md` replaces them, and only `lib/ctl.py` and the ids in the journeys change.
