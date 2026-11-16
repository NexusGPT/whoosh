#!/usr/bin/env python3
"""DERIVE THE LANE PARTITION FROM THE FOLDER TREE.

    python3 scripts/partition.py [--features Sources/Features] [--out docs/lane-dag.json] [--check]
    python3 scripts/partition.py --selftest

repo-shape.md rule 2. Each folder under the features folder is one lane, and the lane owns that folder.
Every file and folder outside every feature folder belongs to the ROOT lane and is frozen for the wave.
The file written has the shape the partition checks read:

    {"derived": true, "features": "<path>", "roots": ["root"],
     "lanes": [{"id": "root", "owns": [...]}, {"id": "<feature>", "owns": ["<path>/<Feature>/**"],
               "depends_on": ["root"]}],
     "generated_at": "<utc>"}

--check exits 1 when the file on disk differs from what the tree derives (a hand edit, or a feature folder
added or removed since). A hand-written partition is not this script's: it carries "derived": false and a
"reason", and --check says so and exits 1.
"""
import argparse
import datetime
import fnmatch
import json
import os
import shutil
import sys
import tempfile

SKIP = {".git", ".build", "DerivedData", "node_modules", ".DS_Store"}


def derive(repo, features):
    feat_abs = os.path.join(repo, features)
    if not os.path.isdir(feat_abs):
        raise SystemExit("REFUSED: no features folder at %s" % feat_abs)
    lanes = []
    names = sorted(n for n in os.listdir(feat_abs)
                   if os.path.isdir(os.path.join(feat_abs, n)) and n not in SKIP and not n.startswith("."))
    ids = set()
    for n in names:
        lid = n.lower()
        if lid in ids or lid == "root":
            raise SystemExit("REFUSED: two feature folders map to the lane id '%s'" % lid)
        ids.add(lid)
        lanes.append({"id": lid, "owns": ["%s/%s/**" % (features, n)], "depends_on": ["root"]})
    # The root lane owns every sibling along the path down to the features folder, never the folder itself.
    root_owns = []
    parts = features.strip("/").split("/")
    here = ""
    for depth, part in enumerate(parts):
        base = os.path.join(repo, here) if here else repo
        for entry in sorted(os.listdir(base)):
            if entry in SKIP or entry == part:
                continue
            rel = "%s/%s" % (here, entry) if here else entry
            root_owns.append(rel + "/**" if os.path.isdir(os.path.join(base, entry)) else rel)
        loose = [e for e in sorted(os.listdir(os.path.join(base, part)))
                 if depth == len(parts) - 1 and os.path.isfile(os.path.join(base, part, e)) and e not in SKIP]
        for e in loose:
            root_owns.append("%s/%s" % (features, e))
        here = "%s/%s" % (here, part) if here else part
    return {"derived": True, "features": features, "roots": ["root"],
            "lanes": [{"id": "root", "owns": root_owns}] + lanes}


def overlaps(a, b):
    """Two owns globs overlap when either matches the other's literal prefix."""
    pa, pb = a.rstrip("*").rstrip("/"), b.rstrip("*").rstrip("/")
    return (pa == pb or pa.startswith(pb + "/") or pb.startswith(pa + "/")
            or fnmatch.fnmatch(pa, b) or fnmatch.fnmatch(pb, a))


def disjoint(dag):
    problems = []
    lanes = dag["lanes"]
    for i, la in enumerate(lanes):
        for lb in lanes[i + 1:]:
            for ga in la["owns"]:
                for gb in lb["owns"]:
                    if overlaps(ga, gb):
                        problems.append("%s owns %s and %s owns %s" % (la["id"], ga, lb["id"], gb))
    return problems


def check_hand(dag, repo, features):
    """A hand-written partition (a wider lane set than the folder tree can derive: core crates, test files)
    holds when its owns are pairwise disjoint, every feature folder on disk is owned by exactly one lane,
    and every feature folder a lane owns exists. It carries "derived": false and a "reason"."""
    problems = []
    if not dag.get("reason"):
        problems.append("a hand-written partition needs a reason")
    lanes = dag.get("lanes", [])
    problems += disjoint({"lanes": lanes})
    feat_abs = os.path.join(repo, features)
    on_disk = sorted(n for n in os.listdir(feat_abs)
                     if os.path.isdir(os.path.join(feat_abs, n)) and n not in SKIP and not n.startswith(".")) if os.path.isdir(feat_abs) else []
    for n in on_disk:
        glob = "%s/%s/**" % (features, n)
        owners = [l["id"] for l in lanes if glob in l.get("owns", [])]
        if len(owners) != 1:
            problems.append("feature folder %s/%s is owned by %d lanes %s" % (features, n, len(owners), owners))
    for l in lanes:
        for g in l.get("owns", []):
            if g.startswith(features + "/") and g.endswith("/**"):
                folder = g[:-3]
                if not os.path.isdir(os.path.join(repo, folder)):
                    problems.append("lane %s owns %s, which does not exist" % (l["id"], g))
    return problems


def comparable(dag):
    return {k: v for k, v in dag.items() if k != "generated_at"}


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=".")
    ap.add_argument("--features", default="Sources/Features")
    ap.add_argument("--out", default="docs/lane-dag.json")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    dag = derive(a.repo, a.features.strip("/"))
    problems = disjoint(dag)
    if problems:
        print("REFUSED: the derived partition overlaps:\n  " + "\n  ".join(problems))
        return 1
    out = os.path.join(a.repo, a.out)
    if a.check:
        if not os.path.isfile(out):
            print("DRIFT: no partition file at %s" % out)
            return 1
        with open(out) as f:
            disk = json.load(f)
        if disk.get("derived") is False:
            problems = check_hand(disk, a.repo, a.features.strip("/"))
            if problems:
                print("HAND-WRITTEN partition %s (reason: %s) does not hold against the tree:\n  %s"
                      % (out, disk.get("reason", "none given"), "\n  ".join(problems)))
                return 1
            print("HAND-WRITTEN partition holds against the tree: %d lanes, every feature folder owned once, owns pairwise disjoint"
                  % len(disk.get("lanes", [])))
            return 0
        if comparable(disk) != comparable(dag):
            print("DRIFT: %s differs from what the tree derives; run partition.py to rewrite it" % out)
            return 1
        print("partition matches the tree: %d feature lanes" % (len(dag["lanes"]) - 1))
        return 0
    # WRITING THE FILE CHANGES THE TREE IT DESCRIBES: the first write may create docs/, which the root
    # lane then owns. So write, re-derive, and write again until the two agree — otherwise --check
    # reports drift against the file it just produced.
    for _ in range(3):
        dag["generated_at"] = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
        with open(out, "w") as f:
            json.dump(dag, f, indent=2)
            f.write("\n")
        again = derive(a.repo, a.features.strip("/"))
        if comparable(again) == comparable(dag):
            break
        dag = again
    print("wrote %s: %d feature lanes, root owns %d entries" % (out, len(dag["lanes"]) - 1, len(dag["lanes"][0]["owns"])))
    return 0


def selftest():
    n = failed = 0

    def ok(cond, what):
        nonlocal n, failed
        n += 1
        if cond:
            print("  PASS  " + what)
        else:
            failed += 1
            print("  FAIL  " + what)

    st = tempfile.mkdtemp(prefix="nx-partition-st.")
    try:
        def touch(rel):
            p = os.path.join(st, rel)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            open(p, "w").close()
        for rel in ("Package.swift", "README.md", "Sources/App/main.swift",
                    "Sources/Features/Chat/ChatView.swift", "Sources/Features/Notch/NotchView.swift",
                    "Sources/Features/Registry.swift", "Tests/Unit/ChatTests.swift"):
            touch(rel)
        dag = derive(st, "Sources/Features")
        ids = [l["id"] for l in dag["lanes"]]
        ok(ids == ["root", "chat", "notch"], "one lane per feature folder plus root (%s)" % ids)
        root = dag["lanes"][0]["owns"]
        ok(root == ["Package.swift", "README.md", "Tests/**", "Sources/App/**", "Sources/Features/Registry.swift"],
           "root owns every sibling on the path and the loose registry file (%s)" % root)
        ok(disjoint(dag) == [], "the derived owns sets are pairwise disjoint")
        bad = {"lanes": [{"id": "a", "owns": ["Sources/**"]}, {"id": "b", "owns": ["Sources/Features/Chat/**"]}]}
        ok(len(disjoint(bad)) == 1, "an overlapping hand partition is caught")
        ok(main(["--repo", st]) == 0 and os.path.isfile(os.path.join(st, "docs/lane-dag.json")), "writes docs/lane-dag.json")
        ok(main(["--repo", st, "--check"]) == 0, "--check passes right after a write")
        touch("Sources/Features/Settings/SettingsView.swift")
        ok(main(["--repo", st, "--check"]) == 1, "--check exits 1 after a feature folder is added")
        with open(os.path.join(st, "docs/lane-dag.json"), "w") as f:
            json.dump({"derived": False, "reason": "seam not cut yet", "lanes": []}, f)
        ok(main(["--repo", st, "--check"]) == 1, "--check exits 1 on a hand-written partition that leaves feature folders unowned")
        hand = {"derived": False, "reason": "core crates and test files are lanes too",
                "lanes": [{"id": "root", "owns": ["Package.swift"]},
                          {"id": "chat", "owns": ["Sources/Features/Chat/**"]},
                          {"id": "notch", "owns": ["Sources/Features/Notch/**", "Tests/Unit/NotchTests.swift"]},
                          {"id": "settings", "owns": ["Sources/Features/Settings/**"]}]}
        with open(os.path.join(st, "docs/lane-dag.json"), "w") as f:
            json.dump(hand, f)
        ok(main(["--repo", st, "--check"]) == 0, "--check passes a hand-written partition that holds against the tree")
        hand["lanes"].append({"id": "ghost", "owns": ["Sources/Features/Ghost/**"]})
        with open(os.path.join(st, "docs/lane-dag.json"), "w") as f:
            json.dump(hand, f)
        ok(main(["--repo", st, "--check"]) == 1, "--check exits 1 when a lane owns a feature folder that does not exist")
    finally:
        shutil.rmtree(st)
    print("%d checks, %d failed, 0 not judged" % (n, failed))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
