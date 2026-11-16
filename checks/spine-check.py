"""Spine check: fn-spine. A view holds no second copy of a server value; this scans the view layer for the signed rule.

The rule is the signed acceptance of fn-spine (docs/plan.json): no @State or @StateObject typed with a CoreBridge type
under Sources/Features. The scan is exactly that:
  1. `@StateObject` anywhere outside Sources/App is red.
  2. `@State` outside Sources/Features/Spine and Sources/App is red only when its declared type, or its initialiser
     expression, names a type declared under Sources/CoreBridge (read from those files at scan time, never a hand list),
     or reaches the core through a `core` receiver (`core.`, `.core.`).
  Visual flags, drafts, measures and handles to a shared observable model are not core values and stay green.
The old `// local-ui` marker rule was the tests-j1 lane's reading, not the signed acceptance; the marker is no longer read.
Repo root: env WHOOSH_REPO, else this repo. Exit 1 when F > 0 or U > 0.
"""
import glob, os, re, sys

ROOT = os.environ.get("WHOOSH_REPO") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STATE_HOME = ("Sources/Features/Spine/", "Sources/App/")
OBJ_HOME = ("Sources/App/",)
RX_OBJ = re.compile(r"@StateObject\b")
RX_STATE = re.compile(r"@State\b(?!Object)")
RX_DECL = re.compile(r"@State\s+(?:(?:private|fileprivate|internal|public)(?:\(set\))?\s+)*var\s+\w+\s*(?::\s*(?P<type>[^=]+?))?\s*(?:=\s*(?P<init>.*))?$", re.S)
RX_CORE_RECEIVER = re.compile(r"(?<![A-Za-z0-9_])core\s*\.|\.core\s*\.")
RX_TYPE_DECL = re.compile(r"^(?:(?:public|internal|final|private|fileprivate|open)\s+)*(?:struct|class|enum|protocol|typealias|actor)\s+([A-Z]\w*)", re.M)


def core_types(root):
    """Every top-level type name declared under Sources/CoreBridge, read from the files. A nested type (CoreHost.Kind) is reached
    through its owner and is not a bare name a view can use for its own member (SnoozeOption.Kind)."""
    names = set()
    for p in glob.glob(os.path.join(root, "Sources/CoreBridge/**/*.swift"), recursive=True):
        names.update(RX_TYPE_DECL.findall(open(p, encoding="utf-8", errors="replace").read()))
    return names


def declaration(lines, i):
    """The @State statement starting at line i (0-based): joined until its brackets balance, at most 12 lines."""
    text, depth = "", 0
    for line in lines[i:i + 12]:
        code = line.split("//")[0]
        text += code + "\n"
        depth += sum(code.count(c) for c in "([{") - sum(code.count(c) for c in ")]}")
        if depth <= 0:
            break
    return text.strip()


def violation(decl, names):
    """The reason a @State declaration holds a core value, or None."""
    m = RX_DECL.search(decl)
    if not m:
        return None
    for part, label in ((m.group("type"), "declared type"), (m.group("init"), "initialiser")):
        if not part:
            continue
        hit = sorted(set(re.findall(r"(?<![\w.])[A-Za-z_]\w*", part)) & names)   # a name after a dot is some other type's member
        if hit:
            return "%s names %s" % (label, ", ".join(hit))
        if label == "initialiser" and RX_CORE_RECEIVER.search(part):
            return "initialiser reads the core"
    return None


def scan(root):
    n, obj, state = 0, [], []
    names = core_types(root)
    for d, _, fs in os.walk(os.path.join(root, "Sources")):
        for f in fs:
            p = os.path.join(d, f); rel = os.path.relpath(p, root)
            if not f.endswith(".swift") or rel.startswith("Sources/CoreBridge/Generated/"):
                continue
            n += 1
            lines = open(p, encoding="utf-8", errors="replace").read().split("\n")
            for i, line in enumerate(lines):
                code = line.split("//")[0]
                if RX_OBJ.search(code) and not rel.startswith(OBJ_HOME):
                    obj.append("%s:%d" % (rel, i + 1))
                if RX_STATE.search(code) and not rel.startswith(STATE_HOME):
                    why = violation(declaration(lines, i), names)
                    if why:
                        state.append("%s:%d (%s)" % (rel, i + 1, why))
    return n, obj, state, len(names)


def main():
    n, obj, state, ntypes = scan(ROOT)
    spine = os.path.join(ROOT, "Sources/Features/Spine")
    names = core_types(ROOT)
    sample = next((x for x in ("InboxRow", "Message") if x in names), sorted(names)[0] if names else "CoreType")
    cs = [
        ("control-core-types-read-from-corebridge", len(names) > 50 and "WhooshCore" in names and "CoreHost" in names, "%d top-level types read" % len(names)),
        ("control-core-typed-state-is-flagged", violation("@State private var rows: [%s] = []" % sample, names) is not None, "sample @State typed with %s" % sample),
        ("control-core-read-initialiser-is-flagged", violation("@State private var n = core.counts()", names) is not None, "sample @State = core.counts()"),
        ("control-visual-flag-is-not-flagged", violation("@State private var hovered = false", names) is None, "sample @State hovered"),
        ("control-member-of-another-type-is-not-flagged", violation("@State private var k: SnoozeOption.Kind? = .laterToday", names) is None, "sample nested Kind"),
        ("scanner-reads-feature-files", n > 0, "%d swift files outside the Spine" % n),
        ("spine-home-exists", os.path.isdir(spine) and any(f.endswith(".swift") for f in os.listdir(spine)), spine),
        ("control-state-pattern", bool(RX_STATE.search("    @State private var hover = false")), "sample @State"),
        ("control-stateobject-pattern", bool(RX_OBJ.search("@StateObject var m = M()")), "sample @StateObject"),
        ("control-state-object-not-state", not RX_STATE.search("@StateObject var m = M()"), "@State must not match @StateObject"),
        ("no-StateObject-outside-spine", not obj, "%d: %s" % (len(obj), ", ".join(obj[:5]))),
        ("no-core-typed-State-outside-spine", not state, "%d: %s" % (len(state), ", ".join(state[:5]))),
    ]
    print("@State/@StateObject outside the Spine rules: %d" % (len(obj) + len(state)))
    print("MUTANTS (a check is green only after it has been seen red on its mutant)\n| check | mutant |\n|---|---|")
    for k, v in [("scanner-reads-feature-files", "WHOOSH_REPO points at an empty tree"), ("control-*", "the type reader, the core-receiver pattern or the member rule is loosened so a control sample changes verdict"), ("spine-home-exists", "the Spine folder is removed"),
                 ("no-StateObject-outside-spine", "a feature view declares @StateObject"),
                 ("no-core-typed-State-outside-spine", "a feature view declares @State private var rows: [InboxRow] = [] (or reads core.) outside the Spine"),
                 ("control-state-pattern", "the @State pattern no longer matches its own sample")]:
        print("| %s | %s |" % (k, v))
    bad = [c for c in cs if not c[1]]
    for l, ok, d in bad:
        sys.stderr.write("FAIL %s: %s\n" % (l, d))
    print("%d checks, %d failed, 0 not judged" % (len(cs), len(bad)))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
