#!/usr/bin/env python3
"""Background-hooks check (lane bg-hooks): the app tells the engine about the battery, the heat and the owner's activity, and
hands it the meaning model only while that model is loaded. Three judges, one verdict shape.

  python3 checks/background-check.py                 judges the SOURCES (Sources/Features/BackgroundWork, the hand-over lines in
                                                     Embedding, the one call line in Shortcuts): the engine calls exist, the floor is the
                                                     engine's rule, nothing polls, no Swift 6 trap, the hand-over and take-back are there
  python3 checks/background-check.py --wiring        adds the lines the lead applies outside this lane (feature registry, `fixture power`
                                                     and `fixture embedder` verbs); red until they are there
  python3 checks/background-check.py --live          drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) with
                                                     `fixture power` and reads the row `mac-main.background`:
                                                     sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/background-check.py --live'
  python3 checks/background-check.py --selftest      the real sources must pass, the live cases must pass against a model of the intended
                                                     behaviour, and each mutant (a source edit, or a model that breaks one behaviour) must fail
                                                     on the case it breaks

Prints 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
The model in --selftest repeats the intended rules in Python so the cases can be proven able to fail; it is not evidence about the app.
"""
import json
import os
import re
import socket
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BG = "Sources/Features/BackgroundWork/"
F = {
    "gate": BG + "ConditionsGate.swift", "ports": BG + "BackgroundPorts.swift", "live": BG + "BackgroundEngineLive.swift",
    "cond": BG + "BackgroundConditions.swift", "hub": BG + "BackgroundWork.swift", "state": BG + "BackgroundState.swift",
    "feature": BG + "BackgroundWorkFeature.swift", "power": BG + "SystemPower.swift",
    "gemma": "Sources/Features/Embedding/GemmaEmbedder.swift", "meaning": "Sources/Features/Embedding/MeaningSearch.swift",
    "mstore": "Sources/Features/Embedding/MeaningModelStore.swift", "mrig": "Sources/Features/Embedding/MeaningModelRig.swift",
    "mweights": "Sources/Features/Embedding/GemmaWeights.swift", "mrow": BG + "MeaningModelRow.swift", "idle": "Sources/Features/Embedding/IdleHolder.swift",
    "keys": "Sources/Features/Shortcuts/ShortcutEngine.swift",
}
WIRING = {
    "registry": "Sources/App/FeatureRegistry.swift", "verbs": "Sources/ControlSocket/FixtureVerbs.swift",
}
CRAWL = "core/crates/engine/src/crawl.rs"
CONSTANTS = "Sources/CoreBridge/Generated/Constants.swift"
REGION = "mac-main.background"


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as fh:
        return fh.read()


def body_of(text, anchor):
    """The brace-balanced body that opens after the first `anchor` in text, or None."""
    i = text.find(anchor)
    if i < 0:
        return None
    j = text.find("{", i)
    if j < 0:
        return None
    depth = 0
    for k in range(j, len(text)):
        depth += {"{": 1, "}": -1}.get(text[k], 0)
        if depth == 0:
            return text[j:k + 1]
    return None


def strip_comments(text):
    return re.sub(r"//[^\n]*", "", text)


# ---------------------------------------------------------------- source judge: (name, ok, why) rows over a {key: text} dict
def judge_sources(t):
    t = {k: strip_comments(v) for k, v in t.items()}
    rows = []

    def add(name, ok, why):
        rows.append((name, bool(ok), why))

    add("engine-conditions-call", re.search(r"\bsetBackgroundConditions\(batteryPercent:", t["live"])
        and re.search(r"engine\.setConditions\(", t["cond"]),
        "the conditions reach set_background_conditions through BackgroundConditions.evaluate")
    add("engine-activity-call", re.search(r"\bnoteOwnerActivity\(\)", t["live"]) and re.search(r"engine\.ownerActed\(\)", t["hub"]),
        "OwnerActivity.note reaches note_owner_activity")
    add("engine-embedder-call", re.search(r"\bsetBackgroundEmbedder\(embedder:", t["live"]), "the hand-over reaches set_background_embedder")
    add("heat-listened", "thermalStateDidChangeNotification" in t["cond"] and "thermalState" in t["power"],
        "a change of heat is heard and read (serious or critical = true)")
    add("serious-or-critical", re.search(r"\.serious\s*\|\|\s*\w+\s*==\s*\.critical", t["power"]), "serious AND critical both count as hot")
    add("power-listened", "IOPSNotificationCreateRunLoopSource" in t["cond"], "a change of battery or charger is heard (no polling)")
    add("no-battery-is-mains", re.search(r"PowerConditions\(batteryPercent:\s*100,\s*charging:\s*true", t["power"]),
        "a Mac with no battery reads charging, 100")
    add("no-polling", not re.search(r"\bTimer\b|scheduledTimer|DispatchSource\.makeTimerSource|while\s+true|Task\.sleep|usleep|\bsleep\(", "".join(
        # the one allowed wait: BackgroundWork.start's single, bounded, test-copy-only hold of the hand-over (not a poll)
        t[k].replace("Thread.sleep(forTimeInterval: Double(waitMs) / 1000)", "") for k in ("cond", "hub", "gate", "state", "power"))),
        "no timer, no loop, no sleep: a held-back change is sent by ONE asyncAfter at the end of the minute")
    add("min-interval-60", re.search(r"CONDITIONS_MIN_INTERVAL_S:\s*TimeInterval\s*=\s*60\b", t["ports"]),
        "a change of charge reaches the engine at most once a minute")
    add("floor-from-constant", "ATTACHMENT_INDEX_BATTERY_MIN" in t["cond"] and "ATTACHMENT_INDEX_BATTERY_MIN" in t["feature"],
        "the battery floor is the ledger's constant, not a number here")
    hold = body_of(t["gate"], "func holdsWork")
    add("hold-rule-matches-engine", hold and re.search(r"thermalSerious\s*\|\|\s*\(\s*!charging\s*&&\s*batteryPercent\s*<\s*floor\s*\)", hold),
        "the row's paused rule is the engine's `held_by`: serious heat, or battery power strictly under the floor")
    dec = body_of(t["gate"], "func decide")
    add("gate-sends-on-change", dec and "now.charging != last.charging" in dec and "now.thermalSerious != last.thermalSerious" in dec
        and "holdsWork(floor: floor) != last.holdsWork(floor: floor)" in dec,
        "a charger change, a heat change and a crossing of the floor are sent at once")
    add("gate-holds-drift", dec and re.search(r"waited\s*>=\s*minInterval\s*\?\s*\.send\s*:\s*\.later", dec), "other changes wait for the minute")
    add("throttle-owner", re.search(r"OWNER_NOTE_MIN_INTERVAL_S:\s*TimeInterval\s*=\s*0\.5", t["ports"])
        and re.search(r"now\s*-\s*lastCall\s*>=\s*minInterval", t["hub"]), "a key repeat is one engine call per half second")
    add("owner-note-async", re.search(r"queue\.async\s*\{[^}]*ownerActed\(\)", t["hub"]), "the engine call never runs on the caller's thread")
    run = body_of(t["keys"], "private func run(")
    add("command-door-notes", run and re.search(r"BackgroundWork\.noteOwnerActivity\(\)", run)
        and run.index("noteOwnerActivity") < run.index("CommandRegistry.handler"),
        "every command, key or click, notes the owner BEFORE its handler runs (ShortcutEngine.run)")
    add("handover-at-launch", re.search(r"static func start\(\)[^\n]*launched", t["hub"]) and re.search(r"parts\.handOver\.start\(\)", t["hub"])
        and re.search(r"engine\.setEmbedder\(embedder\)", t["hub"]) and "sharedEmbedder" in t["hub"],
        "the app's one embedder is handed to the engine at launch, off the main thread (the object holds no model)")
    add("launch-loads-nothing", "sharedEmbedder" in t["meaning"] and not re.search(r"\.embed(Query|Titled|DocumentBatch)?\(", t["hub"])
        and "loadContainer" not in t["hub"], "the hand-over calls no embed and no load")
    add("keeps-embedder-on-unload", not re.search(r"embedderUnloaded|BackgroundWork\.", t["gemma"]),
        "the model's unload does not take the embedder back: the engine keeps the object, the holder frees the model after EMBED_IDLE_UNLOAD_S")
    add("missing-model-said", re.search(r"state\.update \{ \$0\.modelMissing = true \}\s*engine\.setEmbedder\(nil\)", t["hub"]) and "weightsPresent" in t["meaning"], "no weights on this Mac: the engine is told (the Sources pane says so in words)")
    add("conditions-hand-nothing-over", "setEmbedder" not in t["cond"] and "setModelMissing" not in t["cond"],
        "the conditions object never touches the embedder")
    add("swift6-observer", not re.search(r"queue:\s*\.main|assumeIsolated", "".join(t[k] for k in ("cond", "hub", "feature"))),
        "no observer on .main and no MainActor.assumeIsolated (main59's compile failure)")
    add("store-reuses-installer", re.search(r"ModelInstaller\(spec:\s*rig\.spec", t["mstore"]) and "installer.start()" in t["mstore"],
        "the download is the existing ModelInstaller (resumable, SHA-256, free-space check), not a second fetcher")
    # the state is set before the watcher task and before the installer starts; the one line allowed between them is the in-flight marker (round 14, 12 Nov: a quit
    # from here on continues at the next launch). The text judged here has no comments (strip_comments), so the pattern names code only.
    add("store-start-state-at-click", re.search(r"set\(\.downloading\(received: ModelDisk\.bytesOnDisk[^\n]*\n(?:\s*markWanted\(true\)[^\n]*\n)?\s*let watch = Task", t["mstore"]),
        "the state is downloading the moment the download starts, before the first byte")
    add("store-progress-timer", "tickNs: UInt64 = 250_000_000" in t["mstore"] and "ModelDisk.bytesOnDisk" in body_of(t["mstore"], "let watch = Task"),
        "progress is read from the bytes on disk every 250 ms")
    add("store-hands-over-when-ready", re.search(r"case \.ready:.*?onReady\(\)", t["mstore"], re.S) and "BackgroundWork.recheckModel()" in t["mstore"],
        "a finished download hands the model to the engine with no relaunch")
    add("rig-loopback-only", "WHOOSH_RIG_EMBED_BASE" in t["mrig"] and "loopback(base)" in t["mrig"] and "runModelsFolder" in t["mweights"],
        "a rig copy downloads into its run folder and only from a loopback address")
    add("door-registered", "MeaningModelDoor.region" in t["feature"] and "clickTargets" in t["feature"] and "mac-settings-ai.meaning.download" in t["mrow"],
        "the row `mac-settings-ai.meaning` and its Download click are served")
    add("region-registered", REGION in t["feature"] and "rowSources" in t["feature"], "the row `%s` is served" % REGION)
    add("launch-hook", re.search(r"static func view\(for slot: SurfaceSlot\)[^\n]*BackgroundWork\.start\(\)", t["feature"]),
        "the first reading is taken from the feature's launch hook")
    return rows


def judge_wiring(t):
    rows = []
    reg = t["registry"]
    rows.append(("wiring-registry", "BackgroundWorkFeature.self" in reg, "FeatureRegistry.all lists BackgroundWorkFeature"))
    v = t["verbs"]
    rows.append(("wiring-fixture-power", re.search(r'case\s*\("power"', v) and "BackgroundWork.simulatePower" in v,
                 "`fixture power <percent> <charging|battery> <nominal|fair|serious|critical>` and `fixture power real` reach BackgroundWork.simulatePower"))
    rows.append(("wiring-fixture-embedder", re.search(r'case\s*\("embedder"', v) and "BackgroundWork.simulateEmbedder" in v,
                 "`fixture embedder loaded|unloaded` reaches BackgroundWork.simulateEmbedder"))
    return [(n, bool(ok), w) for n, ok, w in rows]


def engine_rule_parity():
    """The engine's rule, read from crawl.rs, and the constant the Swift side reads, read from Constants.swift."""
    rs, sw = read(CRAWL), read(CONSTANTS)
    held = re.search(r"fn held_by\(c: Conditions\) -> bool \{\s*(.*?)\s*\}", rs, re.S)
    ok = bool(held) and "c.thermal_serious" in held.group(1) and "!c.charging" in held.group(1) and "< ATTACHMENT_INDEX_BATTERY_MIN" in held.group(1)
    return [("engine-rule-unchanged", ok, "crawl.rs held_by is still `thermal || (!charging && percent < MIN)`, the rule the Swift row repeats"),
            ("floor-constant-exists", re.search(r"ATTACHMENT_INDEX_BATTERY_MIN:\s*Int\s*=\s*50\b", sw), "the ledger constant is 50")]


# ---------------------------------------------------------------- live cases, one driver interface (socket or model)
class Socket:
    def __init__(self):
        d = os.environ.get("WHOOSH_SOCKET_DIR", "")
        self.path = os.path.join(d, "whoosh.sock")
        self.s, self.buf = None, b""

    def send(self, verb, *args):
        if self.s is None:
            if not os.path.exists(self.path):
                raise RuntimeError("no socket at %s (the app is not running)" % self.path)
            self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.s.settimeout(5)
            self.s.connect(self.path)
        self.s.sendall((json.dumps({"verb": verb, "args": [str(a) for a in args]}) + "\n").encode())
        while b"\n" not in self.buf:
            chunk = self.s.recv(65536)
            if not chunk:
                raise RuntimeError("socket closed")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        return json.loads(line.decode())

    def row(self):
        r = self.send("rows", REGION)
        if r.get("ok") is not True or len(r.get("rows", [])) != 1:
            raise RuntimeError("rows %s: %s" % (REGION, r.get("error", r)))
        return r["rows"][0]

    def power(self, *words):
        r = self.send("fixture", "power", *words)
        if r.get("ok") is not True:
            raise RuntimeError("fixture power %s: %s" % (" ".join(map(str, words)), r.get("error")))

    def embedder(self, word):
        r = self.send("fixture", "embedder", word)
        if r.get("ok") is not True:
            raise RuntimeError("fixture embedder %s: %s" % (word, r.get("error")))

    def key(self, chord):
        self.send("key", chord)            # ok:false (the key did nothing here) is fine: the owner still acted

    def settle(self):
        time.sleep(0.4)


def cases(d):
    """Each case: (name, ok, why). A driver exception is not judged (the caller counts it)."""
    out = []
    d.power("80", "charging", "nominal"); d.settle()
    base = d.row()
    out.append(("start-reads-power", base["source"] == "fixture" and base["sent"] is True and base["crawl"] == "running",
                "after `fixture power 80 charging nominal` the engine was told and the crawl runs"))

    d.power("30", "battery", "nominal"); d.settle()
    low = d.row()
    out.append(("low-battery-pauses", low["crawl"] == "paused" and low["reason"] == "battery" and low["sent_battery_percent"] == 30
                and low["sent_charging"] is False and low["sends"] == base["sends"] + 1,
                "30 percent on battery: told at once, paused for battery (floor %s)" % low["floor"]))

    d.power("30", "charging", "nominal"); d.settle()
    plugged = d.row()
    out.append(("plugging-in-resumes", plugged["crawl"] == "running" and plugged["reason"] == "" and plugged["sent_charging"] is True
                and plugged["sends"] == low["sends"] + 1, "plugged in at 30 percent: told at once, running again"))

    d.power("50", "battery", "nominal"); d.settle()
    at_floor = d.row()
    d.power("49", "battery", "nominal"); d.settle()
    under = d.row()
    out.append(("floor-is-strict", at_floor["crawl"] == "running" and under["crawl"] == "paused" and under["reason"] == "battery",
                "50 percent on battery still runs (the engine holds under the floor, not at it); 49 pauses"))

    d.power("90", "charging", "serious"); d.settle()
    hot = d.row()
    out.append(("heat-pauses", hot["crawl"] == "paused" and hot["reason"] == "heat" and hot["sent_thermal_serious"] is True,
                "serious heat: paused for heat even on mains"))
    d.power("90", "charging", "nominal"); d.settle()
    cool = d.row()
    out.append(("cooling-resumes", cool["crawl"] == "running" and cool["sent_thermal_serious"] is False, "nominal again: running"))

    d.power("90", "charging", "critical"); d.settle()
    crit = d.row()
    out.append(("critical-counts-as-heat", crit["crawl"] == "paused" and crit["reason"] == "heat", "critical is serious or worse: paused"))
    d.power("90", "charging", "nominal"); d.settle()

    before = d.row()
    d.power("89", "charging", "nominal"); d.settle()
    drift = d.row()
    out.append(("small-drift-not-flooded", drift["sends"] == before["sends"] and drift.get("pending_ms", 0) > 0,
                "a 1 percent drift on mains is held back (pending) and not sent now: never more than once a minute"))

    d.power("90", "charging", "nominal"); d.settle()
    n0 = d.row()
    d.key("j"); d.settle()
    n1 = d.row()
    out.append(("owner-action-noted", n1["owner_notes"] >= n0["owner_notes"] + 1 and n1["owner_calls"] >= n0["owner_calls"] + 1,
                "one key through the command door: the owner was noted and the engine was called"))
    for _ in range(5):
        d.key("j")
    d.settle()
    n2 = d.row()
    out.append(("owner-calls-throttled", n2["owner_notes"] >= n1["owner_notes"] + 5 and n2["owner_calls"] <= n1["owner_calls"] + 2,
                "five quick keys are five notes but at most two engine calls (one per half second)"))

    e0 = d.row()
    d.embedder("loaded"); d.settle()
    e1 = d.row()
    out.append(("model-handed-over-when-loaded", e1["embedder"] == "handed" and e1["handovers"] == e0["handovers"] + 1,
                "model loaded: the engine holds it"))
    d.embedder("unloaded"); d.settle()
    e2 = d.row()
    out.append(("model-taken-back-when-unloaded", e2["embedder"] == "held" and e2["takebacks"] == e1["takebacks"] + 1,
                "model unloaded: the engine holds nothing"))
    d.power("real"); d.settle()
    out.append(("power-real-again", d.row()["source"] == "real", "`fixture power real` gives the system's own reading back"))
    return out


class Model:
    """The intended behaviour, in Python, for --selftest. A mutant breaks exactly one thing."""
    FLOOR, MIN_S = 50, 60.0

    def __init__(self, mutant=""):
        self.m = mutant
        self.reading = None
        self.sent = None
        self.sends = 0
        self.pending = None
        self.notes = self.calls = 0
        self.handed = False
        self.handovers = self.takebacks = 0
        self.source = "real"
        self.last_sent_at = None
        self.t = 0.0
        self.last_call = -1e9

    def held(self, c):
        pct, charging, hot = c
        if self.m == "heat-ignored":
            hot = False
        if self.m == "floor-inclusive":
            return hot or (not charging and pct <= self.FLOOR)
        if self.m == "ignores-charger":
            return hot or pct < self.FLOOR
        return hot or (not charging and pct < self.FLOOR)

    def power(self, *w):
        if w == ("real",):
            self.source = "real"
            return
        pct, how, th = int(w[0]), w[1], w[2]
        c = (pct, how == "charging", th in ("serious", "critical") and not (self.m == "critical-ignored" and th == "critical"))
        self.source = "fixture"
        self.reading = c
        self.t += 1.0
        if self.sent is None:
            send = True
        elif c == self.sent:
            send = False
            self.pending = None
        else:
            now_held, was = self.held(c), self.held(self.sent)
            instant = c[1] != self.sent[1] or c[2] != self.sent[2] or now_held != was
            if self.m == "late-send":
                instant = False
            waited = self.t - (self.last_sent_at or 0)
            send = instant or waited >= self.MIN_S or self.m == "floods"
            self.pending = None if send else int((self.MIN_S - waited) * 1000)
        if send and self.m != "never-tells":
            self.sent, self.sends, self.last_sent_at, self.pending = c, self.sends + 1, self.t, None

    def embedder(self, word):
        if word == "loaded" and self.m != "no-handover":
            self.handed, self.handovers = True, self.handovers + 1
        if word == "unloaded":
            self.handed = False if self.m != "no-takeback" else self.handed
            self.takebacks += 1

    def key(self, chord):
        if self.m == "owner-not-noted":
            return
        self.notes += 1
        self.t += 0.05
        if self.t - self.last_call >= 0.5 or self.m == "owner-unthrottled":
            self.calls += 1
            self.last_call = self.t
        if self.m == "owner-unthrottled":
            self.calls = self.notes

    def row(self):
        s = self.sent
        held = bool(s) and self.held(s)
        r = {"source": self.source, "battery_percent": (self.reading or (-1,))[0], "sent": s is not None,
             "sent_battery_percent": s[0] if s else -1, "sent_charging": s[1] if s else False, "sent_thermal_serious": s[2] if s else False,
             "sends": self.sends, "crawl": "paused" if held else "running",
             "reason": ("heat" if s[2] else "battery") if held else "", "owner_notes": self.notes, "owner_calls": self.calls,
             "embedder": "handed" if self.handed else "held", "handovers": self.handovers, "takebacks": self.takebacks, "floor": self.FLOOR}
        if self.pending is not None:
            r["pending_ms"] = self.pending
        return r

    def settle(self):
        self.t += 0.6


class ModelDriver(Model):
    pass


# ---------------------------------------------------------------- selftest
MODEL_MUTANTS = {          # mutant -> the case that must go red
    "never-tells": "start-reads-power", "floor-inclusive": "floor-is-strict", "ignores-charger": "plugging-in-resumes",
    "heat-ignored": "heat-pauses", "critical-ignored": "critical-counts-as-heat", "floods": "small-drift-not-flooded",
    "owner-not-noted": "owner-action-noted", "owner-unthrottled": "owner-calls-throttled", "no-handover": "model-handed-over-when-loaded",
    "no-takeback": "model-taken-back-when-unloaded",
}
SOURCE_MUTANTS = [          # (name, file key, regex to replace, replacement, the check that must go red)
    ("drop-conditions-call", "cond", r"engine\.setConditions\(", "_ = (", "engine-conditions-call"),
    ("drop-owner-note", "keys", r"BackgroundWork\.noteOwnerActivity\(\)", "()", "command-door-notes"),
    ("note-after-handler", "keys", r"(BackgroundWork\.noteOwnerActivity\(\)[^\n]*\n)(.*?)(guard let handler[^\n]*\n)", r"\2\3\1", "command-door-notes"),
    ("drop-launch-handover", "hub", r"parts\.handOver\.start\(\)", "()", "handover-at-launch"),
    ("launch-embeds", "hub", r"let embedder = find\(\) \{", "let embedder = find() { _ = try? embedder.embed(texts: [\"x\"])", "launch-loads-nothing"),
    ("takeback-on-unload", "gemma", r"onUnload: \{ MLX\.Memory\.clearCache\(\) \}", "onUnload: { MLX.Memory.clearCache(); BackgroundWork.embedderUnloaded() }", "keeps-embedder-on-unload"),
    ("drop-missing-model", "hub", r"engine\.setEmbedder\(nil\)   // \"no model", "()   // \"no model", "missing-model-said"),
    ("floor-inclusive", "gate", r"batteryPercent < floor", "batteryPercent <= floor", "hold-rule-matches-engine"),
    ("floor-hardcoded", "feature", r"WhooshConstants\.ATTACHMENT_INDEX_BATTERY_MIN", "50", "floor-from-constant"),
    ("poll-with-timer", "cond", r"func start\(\) \{", "func start() { _ = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { _ in }", "no-polling"),
    ("observer-on-main", "cond", r"queue: nil", "queue: .main", "swift6-observer"),
    ("drop-heat-listener", "cond", r"forName: ProcessInfo\.thermalStateDidChangeNotification", "forName: ProcessInfo.thermalStateGone", "heat-listened"),
    ("critical-forgotten", "power", r"\|\| state == \.critical", "", "serious-or-critical"),
    ("owner-note-inline", "hub", r"queue\.async \{ \[engine\] in engine\.ownerActed\(\) \}", "engine.ownerActed()", "owner-note-async"),
    ("gate-ignores-charger", "gate", r"now\.charging != last\.charging \|\| ", "", "gate-sends-on-change"),
    ("store-no-start-state", "mstore", r"        set\(\.downloading\(received: ModelDisk\.bytesOnDisk\(spec, in: directory\), total: spec\.totalBytes\)\)[^\n]*\n", "", "store-start-state-at-click"),
    ("store-no-timer", "mstore", r"tickNs: UInt64 = 250_000_000", "tickNs: UInt64 = 0", "store-progress-timer"),
    ("store-no-handover", "mstore", r"onReady\(\); return s", "return s", "store-hands-over-when-ready"),
    ("store-own-fetcher", "mstore", r"ModelInstaller\(spec: rig\.spec", "FileFetcherX(spec: rig.spec", "store-reuses-installer"),
    ("rig-any-address", "mrig", r"loopback\(base\)", "true", "rig-loopback-only"),
    ("conditions-touch-embedder", "cond", r"func evaluate\(\) \{", "func evaluate() { engine.setEmbedder(nil)", "conditions-hand-nothing-over"),
]


def load_sources():
    return {k: read(v) for k, v in F.items()}


def run_rows(rows, label):
    failed = 0
    for name, ok, why in rows:
        print("%s %-34s %s" % ("ok  " if ok else "FAIL", name, why))
        failed += 0 if ok else 1
    return len(rows), failed


def selftest():
    n = f = 0
    base = load_sources()
    rows = judge_sources(base) + engine_rule_parity()
    a, b = run_rows(rows, "sources")
    n, f = n + a, f + b
    live = cases(Model())
    a, b = run_rows(live, "model")
    n, f = n + a, f + b
    print("\nMUTANTS (each must turn its own check red and no other source check red)")
    missed = 0
    for name, key, pat, rep, check in SOURCE_MUTANTS:
        text = base[key]
        mutated, count = re.subn(pat, rep, text, count=1, flags=re.S)
        if not count:
            print("NOT APPLIED  %-22s pattern not found in %s" % (name, F[key]))
            missed += 1
            continue
        t = dict(base)
        t[key] = mutated
        red = {r[0] for r in judge_sources(t) if not r[1]}
        ok = check in red
        print("%s %-22s red: %s" % ("red " if ok else "MISSED", name, ", ".join(sorted(red)) or "none"))
        missed += 0 if ok else 1
    for mutant, check in MODEL_MUTANTS.items():
        red = {c[0] for c in cases(Model(mutant)) if not c[1]}
        ok = check in red
        print("%s %-22s red: %s" % ("red " if ok else "MISSED", "model:" + mutant, ", ".join(sorted(red)) or "none"))
        missed += 0 if ok else 1
    n += len(SOURCE_MUTANTS) + len(MODEL_MUTANTS)
    f += missed
    print("\n%d checks, %d failed, 0 not judged" % (n, f))
    return 1 if f else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    rows = judge_sources(load_sources()) + engine_rule_parity()
    unjudged = 0
    if "--wiring" in argv:
        rows += judge_wiring({k: read(v) for k, v in WIRING.items()})
    if "--live" in argv:
        try:
            rows += cases(Socket())
        except (RuntimeError, OSError, ValueError, KeyError) as e:
            print("NOT JUDGED live cases: %s" % e)
            unjudged += 1
    n, f = run_rows(rows, "run")
    print("\n%d checks, %d failed, %d not judged" % (n, f, unjudged))
    return 1 if f or unjudged else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
