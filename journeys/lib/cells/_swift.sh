# Shared by the Swift test cells. The tests are SwiftPM Swift Testing functions in the package's one test target
# (Package.swift: WhooshTests over Tests/), so they run with `swift test`, not xcodebuild: the xcodegen project (apps/mac/project.yml)
# has the one scheme Whoosh and no test scheme, and the package scheme names are not stable. Free @Test functions carry no suite,
# so a folder is selected by the test names found in its files (read at run time, never a hand list). A test inside a suite has the id
# `WhooshTests.<Type path>/<name>()` (Tests/Contract: `extension Contract { @Suite struct ClickVerb { @Test func ...`), a free function `WhooshTests.<name>()`:
# the filter takes both (round main48: the Contract filter matched none and the cell read NOT JUDGED). Tests/Contract nest TWO types deep
# (`Contract/ClickVerb/name()`), so the type path is `(seg/)*`, not `(seg/)?`: with `?` the round main76 Contract filter selected none again (swift-tests lane).
#   swift_cell <folder under Tests/>   prints FAIL lines and the summary line; returns the cell's exit code
swift_cell() {
  _folder="$1"; _dir="Tests/$1"
  [ -d "$_dir" ] || { unjudged "$_dir does not exist on this branch, so no Swift test of that folder can run"; }
  # One test target over Tests/: two files with the SAME NAME in different folders are one output path for SwiftPM, and the whole
  # target refuses to build ("multiple producers: Compiling Swift Module 'WhooshTests'", main53 to main55: Tests/Unit and
  # Tests/Support both held a FixtureCoreTests.swift). Name the pair here, before swift test hides it behind a crash with no summary.
  _dups=$(find Tests -name '*.swift' -type f | sed 's|.*/||' | sort | uniq -d)
  if [ -n "$_dups" ]; then
    printf 'FAIL two files share a name inside the one test target (SwiftPM builds neither): %s\n' "$(printf '%s' "$_dups" | paste -sd' ' -)"
    find Tests -name '*.swift' -type f | grep -F "$(printf '%s\n' "$_dups" | head -1)" | sed 's/^/FAIL   | /'
    SW_N=0; SW_F=1; return 1
  fi
  set -- "$_dir"/*.swift
  [ -f "$1" ] || { unjudged "no Swift file under $_dir"; }
  _names=$(grep -hoE '@Test.*func +[A-Za-z0-9_]+' "$@" | sed -E 's/.*func +//' | sort -u | paste -sd'|' -)
  [ -n "$_names" ] || { unjudged "no @Test function found under $_dir"; }
  command -v swift > /dev/null 2>&1 || { unjudged "swift is not on the PATH of this run"; }
  # swift test has no app bundle, so FixtureFile.locate() finds the fixture only through WHOOSH_FIXTURE_PATH; without it CoreHost
  # falls back to UnavailableCore and a seam test judges nothing real. Export it, and stop if the file is missing.
  _fx="$(pwd)/docs/design/fixtures/mac-main-sample.json"
  [ -f "$_fx" ] || { unjudged "the design fixture is missing at $_fx"; }
  _out=$(WHOOSH_FIXTURE_CORE=1 WHOOSH_FIXTURE_PATH="$_fx" swift test -j "${WHOOSH_JOBS:-2}" --no-parallel --filter "^WhooshTests\.([A-Za-z0-9_.]+/)*(${_names})\(" 2>&1); _ec=$?
  # a usage problem of the runner itself is not a verdict on the tests
  if printf '%s\n' "$_out" | grep -qiE "unknown option|unexpected argument|usage: swift|invalid value|no tests found|no matching test"; then
    unjudged "swift test could not select the tests of $_dir: $(printf '%s\n' "$_out" | grep -iE 'unknown option|unexpected argument|usage: swift|invalid value|no tests found|no matching test' | head -1)"
  fi
  # the raw output is kept beside the round's cell (the round copy is deleted by the next job): a red cell must name EVERY failing test
  # (round main48 printed the first 8 lines of a run with dozens of issues, and a crash with no summary read as "1 failed")
  _raw="${D:-${TMPDIR:-/tmp}}/swift-test-$_folder.log"; printf '%s\n' "$_out" > "$_raw"
  printf '%s\n' "$_out" | grep -E '^(✘|.*: error:|error:)' | grep -vE '^✘ Test run with' | sed 's/^/FAIL /'
  _run=$(printf '%s\n' "$_out" | grep -E 'Test run with [0-9]+ tests?' | tail -1)
  _n=$(printf '%s' "$_run" | sed -E 's/.*Test run with ([0-9]+) tests?.*/\1/')
  case "$_n" in ''|*[!0-9]*)
    printf 'FAIL swift test gave no test summary (exit %s; raw output %s); last lines follow, the run may have crashed before it finished\n' "$_ec" "$_raw"
    printf '%s\n' "$_out" | tail -12 | cut -c1-240 | sed 's/^/FAIL   | /'
    SW_N=0; SW_F=1; return 1 ;;
  esac
  # one line per failing TEST: "✘ Test name() failed after N seconds with K issues." (an issue line also says "Expectation failed", so " failed" alone over-counts)
  _f=$(printf '%s\n' "$_out" | grep -cE '^✘ Test .*\) failed after ')
  case "$_run" in *failed*) [ "$_f" -gt 0 ] || _f=1 ;; esac
  SW_N=$_n; SW_F=$_f
  [ "$_f" -eq 0 ] && [ "$_ec" -eq 0 ]
}
