#!/usr/bin/env python3
"""Feature-commit guard (tests-r58, plan row J1.58): a case that tests a feature declares the commit that brought it. When that commit is not an ancestor
of the commit the build under test was made from, the case is NOT JUDGED ("feature commit X not in this build"), never red: main63 ran J1.58 on a build
that lacked 2e79dd05 and read red on a missing feature.

    import featurecommit as FC                       (sys.path gets checks/checklib; the name `lib` is journeys/lib)
    s.expect(name, FC.gate("2e79dd05", thunk), want)  one case
    g = FC.Gate("2e79dd05"); s.expect(name, g(thunk), want)   a group

The build's commit is read from, in order: env WHOOSH_BUILD_COMMIT; the Info.plist key NexusBuildCommit of the bundle in env WHOOSH_BUNDLE (scripts/build.sh
writes it). No stamp = the build cannot be told: the case is judged as before (a guard that hid every case would be worse than none).
    python3 checks/checklib/featurecommit.py --selftest     the guard against a scratch repository, then against two mutants: each must go red
"""
import os
import plistlib
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
if os.path.join(ROOT, "journeys") not in sys.path:
    sys.path.insert(0, os.path.join(ROOT, "journeys"))


def build_commit(env=None):
    env = os.environ if env is None else env
    if env.get("WHOOSH_BUILD_COMMIT"):
        return env["WHOOSH_BUILD_COMMIT"].strip()
    b = env.get("WHOOSH_BUNDLE")
    if b:
        try:
            with open(os.path.join(b, "Contents", "Info.plist"), "rb") as f:
                return (plistlib.load(f).get("NexusBuildCommit") or "").strip() or None
        except (OSError, ValueError):
            return None
    return None


def git_is_ancestor(feature, build, repo=ROOT):
    """True / False; None when git cannot tell (a commit the repository does not hold)."""
    p = subprocess.run(["git", "-C", repo, "merge-base", "--is-ancestor", feature, build], capture_output=True, text=True)
    return True if p.returncode == 0 else False if p.returncode == 1 else None


def in_build(feature, build=None, repo=ROOT, is_ancestor=git_is_ancestor):
    """True: the build has the feature. False: it lacks it. None: the build cannot be told."""
    build = build or build_commit()
    if not build:
        return None
    return is_ancestor(feature, build, repo)


class Gate:
    def __init__(self, feature, build=None, repo=ROOT, is_ancestor=git_is_ancestor):
        self.feature, self.build, self.repo, self.is_ancestor = feature, build, repo, is_ancestor

    def __call__(self, thunk):
        def run():
            from lib.ctl import Unreadable
            if in_build(self.feature, self.build, self.repo, self.is_ancestor) is False:
                raise Unreadable("feature commit %s not in this build (%s)" % (self.feature, (self.build or build_commit() or "?")[:8]))
            return thunk()
        return run


def gate(feature, thunk, **kw):
    return Gate(feature, **kw)(thunk)


# ---------------------------------------------------------------- selftest
def selftest():
    import tempfile
    from lib.ctl import Unreadable
    d = tempfile.mkdtemp(prefix="fc-selftest-")
    def g(*a):
        return subprocess.run(["git", "-C", d] + list(a), capture_output=True, text=True, check=True).stdout.strip()
    g("init", "-q"); g("config", "user.email", "t@t"); g("config", "user.name", "t")
    for n in ("a", "b", "c"):
        open(os.path.join(d, n), "w").write(n); g("add", n); g("commit", "-q", "-m", n)
    a, b, c = g("rev-parse", "HEAD~2"), g("rev-parse", "HEAD~1"), g("rev-parse", "HEAD")

    def cases(is_anc):
        """(name, got, want): the guard on a build made at b."""
        out = []
        def result(feature, build=b):
            try:
                return "ran %s" % Gate(feature, build, d, is_anc)(lambda: 7)()
            except Unreadable as e:
                return "not judged: %s" % e
        out.append(("older feature runs", result(a), "ran 7"))
        out.append(("same commit runs", result(b), "ran 7"))
        out.append(("newer feature is NOT JUDGED", result(c).split(":")[0], "not judged"))
        out.append(("the words name the commit", c[:8] in result(c), True))
        out.append(("unknown stamp runs the case", "ran 7" == result(a, build=None) if build_commit({}) is None else True, True))
        out.append(("a commit the repository lacks runs the case", result("0" * 40), "ran 7"))
        return out

    bad = 0
    for n, got, want in cases(git_is_ancestor):
        ok = got == want; bad += not ok
        print("%-44s %s%s" % (n, "ok" if ok else "FAIL", "" if ok else "  got %r want %r" % (got, want)))
    for name, fake in (("always-ancestor", lambda f, bd, r: True), ("never-ancestor", lambda f, bd, r: False)):
        red = [n for n, got, want in cases(fake) if got != want]
        print("mutant %-16s -> %s" % (name, "RED on " + ", ".join(red) if red else "NOT RED"))
        bad += not red
    print("%d problems" % bad)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else 0)
