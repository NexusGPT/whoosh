#!/usr/bin/env python3
"""devicesync: Mac-to-iPhone encrypted sync, QR pairing and the icon file (fn-devicesync, fn-icon-file-sync).

    python3 checks/devicesync-check.py            run every check; exit 1 when F > 0 or U > 0
    python3 checks/devicesync-check.py --selftest every check must pass on the real sources and go RED on its mutant

The sync layer runs against FakeCloudContainer, a local stand-in behind the CloudSync port. The check compiles the
folder Sources/Features/DeviceSync (without ContractBridge.swift) together with a driver and runs it. Nothing touches
iCloud. checks/parity/par-devicesync.py reuses this harness for the parity group.
Prints `N checks, F failed, U not judged`."""
import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "Sources", "Features", "DeviceSync")
BRIDGE = "ContractBridge.swift"
FEATURE_STUB = "DeviceSyncFeature.swift"   # needs FeatureModule from the app target; not part of the standalone build

FUNCTIONAL_IDS = [
    "F-settings-sync", "F-sealed-payload", "F-credential-refused", "F-no-credential-scan", "F-local-only",
    "F-token-single-use", "F-token-expiry", "F-token-replaced", "F-token-qr-only", "F-tamper-refused", "F-lww",
    "F-icon-upload-travels", "F-library-icon-id-only", "S-no-cloud-call", "S-no-literal-expiry",
]
PARITY_IDS = [
    "P-mac-to-mac", "P-iphone-accounts-settings", "P-qr-accounts", "P-retry-offline", "P-backoff-steps",
    "P-retry-coalesce", "P-pull-offline",
]

DRIVER = r'''
import Foundation

final class VClock { var ms: Int64 = 1_000_000; func advance(_ s: Double) { ms += Int64(s * 1000) } }
func check(_ id: String, _ ok: Bool, _ detail: String = "") { print("CHECK \(id) \(ok ? "PASS" : "FAIL") \(detail)") }

func run(_ id: String, _ body: () throws -> Void) { do { try body() } catch { check(id, false, "threw \(error)") } }

let mode = CommandLine.arguments[1]
let expiry = Int(CommandLine.arguments[2])!
let keys = InMemoryKeychain()

func device(_ name: String, _ cloud: FakeCloudContainer, _ clock: VClock) -> DeviceSyncEngine {
    DeviceSyncEngine(deviceId: name, port: cloud, keys: keys, retry: RetryPolicy(backoffSeconds: [1, 2, 4]), clock: { clock.ms })
}
func throwsRefusal(_ body: () throws -> Void) -> SyncRefusal? {
    do { try body(); return nil } catch let e as SyncRefusal { return e } catch { return nil }
}

if mode == "functional" {
    run("F-settings-sync") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock), b = device("iphone-B", cloud, clock)
        try a.write(kind: "split_tab", id: "tab1", fields: ["name": "Newsletters", "rule": "list-header"])
        try b.pull()
        check("F-settings-sync", b.row(kind: "split_tab", id: "tab1")?.fields == ["name": "Newsletters", "rule": "list-header"]
              && a.row(kind: "split_tab", id: "tab1") != nil, "B reads A's write")
    }
    run("F-sealed-payload") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock)
        let rows: [(String, String, [String: String])] = [
            ("account", "acc1", ["address": "owner@example.test", "color": "blue"]),
            ("vip", "v1", ["address": "boss@example.test"]), ("template", "t1", ["name": "Thanks", "body": "Thank you very much"]),
            ("rule", "r1", ["condition": "from-newsletter"]), ("note", "n1", ["text": "call the plumber"]),
            ("decision_model", "d1", ["provider": "local", "model": "gemma"]), ("voice_profile", "p1", ["style": "short and warm"]),
            ("setting", "s1", ["undo_send": "on"]),
        ]
        for r in rows { try a.write(kind: r.0, id: r.1, fields: r.2) }
        let raw = cloud.rawRecords
        var leaked = ""
        for rec in raw {
            let text = String(decoding: rec.payload, as: UTF8.self)
            for r in rows where r.0 == rec.recordKind {
                for (k, v) in r.2 where text.contains(k) || text.contains(v) { leaked = "\(rec.recordKind) shows \(k)/\(v)" }
            }
            if (try? JSONSerialization.jsonObject(with: rec.payload)) != nil { leaked = "\(rec.recordKind) payload is plain JSON" }
        }
        check("F-sealed-payload", raw.count == rows.count && leaked.isEmpty, leaked.isEmpty ? "\(raw.count) rows, all sealed" : leaked)
    }
    run("F-credential-refused") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock)
        let e1 = throwsRefusal { try a.write(kind: "account", id: "x", fields: ["address": "o@example.test", "password": "hunter2"]) }
        let e2 = throwsRefusal { try a.write(kind: "account", id: "x", fields: ["address": "o@example.test", "refresh_token": "r"]) }
        let e3 = throwsRefusal { try a.write(kind: "account", id: "x", fields: ["note": "ya29.abcdef"]) }
        let e4 = throwsRefusal { try a.write(kind: "decision_model", id: "d", fields: ["api_key": "k"]) }
        let ok = throwsRefusal { try a.write(kind: "account", id: "x", fields: ["address": "o@example.test", "keychain_service": "whoosh.imap", "keychain_account": "o@example.test"]) }
        check("F-credential-refused", e1 != nil && e2 != nil && e3 != nil && e4 != nil && ok == nil && cloud.rawRecords.count == 1,
              "refused password, refresh_token, bearer value, api_key; kept a Keychain reference; container holds \(cloud.rawRecords.count)")
    }
    run("F-no-credential-scan") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock), b = device("iphone-B", cloud, clock)
        let secrets = ["hunter2-PASSWORD", "ya29.SECRETTOKEN", "ref-r3fresh"]
        _ = throwsRefusal { try a.write(kind: "account", id: "bad", fields: ["address": "o@example.test", "password": secrets[0]]) }
        try a.write(kind: "account", id: "acc1", fields: ["address": "o@example.test", "keychain_service": "whoosh.imap", "keychain_account": "o@example.test"])
        try b.pull()
        var hit: String? = nil
        for rec in cloud.rawRecords { if let h = CredentialGuard.scan(rec.payload, secrets: secrets) { hit = h } }
        for r in b.rows(ofKind: "account") { for (k, v) in r.fields { if let h = CredentialGuard.scan(Data("\(k)=\(v)".utf8), secrets: secrets) { hit = h } } }
        let control = CredentialGuard.scan(Data("password=\(secrets[0])".utf8), secrets: secrets) != nil
        check("F-no-credential-scan", hit == nil && control && b.rows(ofKind: "account").count == 1, "scan clean: \(hit ?? "nothing found"); control sees a planted secret: \(control)")
    }
    run("F-local-only") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock)
        let l1 = throwsRefusal { try a.write(kind: "layout_state", id: "w", fields: ["pane": "300"]) }
        let l2 = throwsRefusal { try a.write(kind: "folder_sync_state", id: "f", fields: ["cursor": "1"]) }
        let ok = throwsRefusal { try a.write(kind: "split_tab", id: "t", fields: ["name": "VIPs"]) }
        check("F-local-only", l1 == .localOnly("layout_state") && l2 != nil && ok == nil && cloud.rawRecords.count == 1, "device-only kinds stay home")
    }
    run("F-token-single-use") {
        let clock = VClock()
        let svc = PairingService(expirySeconds: expiry, clock: { clock.ms })
        let t = svc.issue()
        let first = svc.redeem(t.value), second = svc.redeem(t.value)
        var firstOK = false; if case .success = first { firstOK = true }
        check("F-token-single-use", firstOK && second == .failure(.alreadyUsed) && !svc.hasLiveToken, "second scan: \(second)")
    }
    run("F-token-expiry") {
        let clock = VClock()
        let svc = PairingService(expirySeconds: expiry, clock: { clock.ms })
        let t1 = svc.issue(); clock.advance(Double(expiry - 1))
        let r1 = svc.redeem(t1.value)
        let t2 = svc.issue(); clock.advance(Double(expiry))
        let r2 = svc.redeem(t2.value)
        let t3 = svc.issue(); clock.advance(Double(expiry + 1))
        let r3 = svc.redeem(t3.value)
        var okFirst = false; if case .success = r1 { okFirst = true }
        check("F-token-expiry", okFirst && r2 == .failure(.expired) && r3 == .failure(.expired), "one second before: ok; at and after QR_EXPIRY_S (\(expiry)): refused")
    }
    run("F-token-replaced") {
        let clock = VClock()
        let svc = PairingService(expirySeconds: expiry, clock: { clock.ms })
        let t1 = svc.issue(), t2 = svc.issue()
        let old = svc.redeem(t1.value), new = svc.redeem(t2.value)
        var newOK = false; if case .success = new { newOK = true }
        check("F-token-replaced", old == .failure(.unknown) && newOK && t1.value != t2.value, "a new link replaces the expired one")
    }
    run("F-token-qr-only") {
        let clock = VClock()
        let svc = PairingService(expirySeconds: expiry, clock: { clock.ms })
        let t = svc.issue()
        check("F-token-qr-only", t.qrPayload == "whoosh-pair:v1:" + t.value && !t.qrPayload.contains("@") && CredentialGuard.scan(Data(t.qrPayload.utf8), secrets: []) == nil && t.value.count >= 32,
              "QR carries the pairing token only")
    }
    run("F-tamper-refused") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock), b = device("iphone-B", cloud, clock)
        try a.write(kind: "note", id: "n1", fields: ["text": "one"])
        try a.write(kind: "note", id: "n2", fields: ["text": "two"])
        let other = cloud.rawRecords.first { $0.id == "n2" }!.payload
        cloud.overwritePayload(kind: "note", id: "n1", with: other)
        var refused = false
        do { try b.pull() } catch let e as SyncRefusal { refused = (e == .tampered) } catch {}
        let cloud2 = FakeCloudContainer(); let a2 = device("mac-A", cloud2, clock), b2 = device("iphone-B", cloud2, clock)
        try a2.write(kind: "note", id: "n1", fields: ["text": "one"]); try b2.pull()
        check("F-tamper-refused", refused && b2.row(kind: "note", id: "n1") != nil, "a sealed blob moved onto another record is refused; untouched record opens")
    }
    run("F-lww") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock), b = device("iphone-B", cloud, clock)
        try a.write(kind: "setting", id: "undo", fields: ["value": "10"]); try b.pull()
        clock.advance(5)
        try b.write(kind: "setting", id: "undo", fields: ["value": "20"])
        try a.pull(); try b.pull()
        let av = a.row(kind: "setting", id: "undo")?.fields["value"], bv = b.row(kind: "setting", id: "undo")?.fields["value"]
        check("F-lww", av == "20" && bv == "20", "later write wins on both devices: A \(av ?? "-"), B \(bv ?? "-")")
    }
    run("F-icon-upload-travels") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock), b = device("iphone-B", cloud, clock)
        let bytes = Data((0..<64).map { UInt8(($0 * 7 + 3) % 251) })
        try a.setAccountIcon(accountId: "acc1", icon: .uploaded(fileId: "file1", bytes: bytes))
        let t0 = clock.ms
        try b.pull()
        let shown = b.accountIcon(accountId: "acc1")
        let assetRaw = cloud.rawRecords.first { $0.recordKind == "account_icon_file" }?.payload ?? Data()
        let plainLeak = assetRaw.range(of: bytes) != nil
        check("F-icon-upload-travels", shown == .uploaded(fileId: "file1", bytes: bytes) && cloud.assetCount == 1 && !plainLeak && clock.ms == t0,
              "B shows the same \(bytes.count) bytes in one pull; assets \(cloud.assetCount); file sealed: \(!plainLeak)")
    }
    run("F-library-icon-id-only") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock), b = device("iphone-B", cloud, clock)
        try a.setAccountIcon(accountId: "acc2", icon: .library(id: "envelope"))
        try b.pull()
        check("F-library-icon-id-only", b.accountIcon(accountId: "acc2") == .library(id: "envelope") && cloud.assetCount == 0 && cloud.rawRecords.count == 1,
              "library icon syncs as its id; asset count \(cloud.assetCount)")
    }
} else {
    run("P-mac-to-mac") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock), b = device("mac-B", cloud, clock), c = device("mac-C", cloud, clock)
        try a.write(kind: "vip", id: "v1", fields: ["address": "boss@example.test"])
        try b.pull()
        let sawOnB = b.row(kind: "vip", id: "v1") != nil
        clock.advance(2)
        try b.write(kind: "vip", id: "v1", fields: ["address": "boss@example.test", "label": "board"])
        try c.pull()
        let sawOnC = c.row(kind: "vip", id: "v1")?.fields["label"] == "board"
        // a conflict: A and C write the same rule in the same millisecond; the larger device id wins everywhere
        try a.write(kind: "rule", id: "r1", fields: ["action": "mute"])
        try c.write(kind: "rule", id: "r1", fields: ["action": "tab"])
        try a.pull(); try b.pull(); try c.pull()
        let winners = [a, b, c].map { $0.row(kind: "rule", id: "r1")?.fields["action"] ?? "-" }
        try a.pull()
        check("P-mac-to-mac", sawOnB && sawOnC && Set(winners).count == 1 && a.row(kind: "vip", id: "v1")?.fields["label"] == "board",
              "three Macs converge: \(winners)")
    }
    run("P-iphone-accounts-settings") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let mac = device("mac-A", cloud, clock), phone = device("iphone-B", cloud, clock)
        try mac.write(kind: "account", id: "acc1", fields: ["address": "one@example.test", "provider": "imap"])
        try mac.write(kind: "account", id: "acc2", fields: ["address": "two@example.test", "provider": "graph"])
        try mac.write(kind: "setting", id: "undo", fields: ["value": "on"])
        try phone.pull()
        let addrs = phone.rows(ofKind: "account").compactMap { $0.fields["address"] }
        check("P-iphone-accounts-settings", addrs == ["one@example.test", "two@example.test"] && phone.row(kind: "setting", id: "undo") != nil && phone.statusLine == "In sync",
              "iPhone shows \(addrs.count) accounts and the setting; status \(phone.statusLine)")
    }
    run("P-qr-accounts") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let svc = PairingService(expirySeconds: expiry, clock: { clock.ms })
        let mac = device("mac-A", cloud, clock), phone = device("iphone-B", cloud, clock)
        try mac.write(kind: "account", id: "acc1", fields: ["address": "one@example.test", "keychain_service": "whoosh.imap", "keychain_account": "one@example.test"])
        let t = svc.issue()
        let scanned = svc.redeem(t.value)
        var scanOK = false; if case .success = scanned { scanOK = true }
        if scanOK { try phone.pull() }
        let again = svc.redeem(t.value)
        check("P-qr-accounts", scanOK && phone.rows(ofKind: "account").count == 1 && again == .failure(.alreadyUsed),
              "accounts arrive after the scan; a second scan is refused")
    }
    run("P-retry-offline") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock), b = device("iphone-B", cloud, clock)
        cloud.online = false
        try a.write(kind: "setting", id: "undo", fields: ["value": "20"])
        var stateOffline = false, line = ""
        if case .offline(let pending, _) = a.state { stateOffline = pending == 1 }
        line = a.statusLine
        var pullFailed = false
        do { try b.pull() } catch { pullFailed = true }
        clock.advance(0.5); a.tick()
        let stillWaiting = a.pendingCount == 1
        clock.advance(0.6); a.tick()          // first wait over, the network is still down
        var gap1: Int64 = 0
        if case .offline(_, let at) = a.state { gap1 = at - clock.ms }
        clock.advance(Double(gap1) / 1000); a.tick()
        var gap2: Int64 = 0
        if case .offline(_, let at) = a.state { gap2 = at - clock.ms }
        cloud.online = true
        clock.advance(Double(gap2) / 1000); a.tick()   // no user action: the timer alone delivers
        var synced = false; if case .inSync = a.state { synced = true }
        try b.pull()
        check("P-retry-offline", stateOffline && line.contains("Offline") && pullFailed && stillWaiting && a.pendingCount == 0 && synced
              && a.statusLine == "In sync" && b.row(kind: "setting", id: "undo")?.fields["value"] == "20",
              "queued while offline (\(line)); delivered on its own after the network returned")
        check("P-backoff-steps", gap1 == 2000 && gap2 == 4000, "waits grow: \(gap1) ms then \(gap2) ms")
    }
    run("P-retry-coalesce") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let a = device("mac-A", cloud, clock), b = device("iphone-B", cloud, clock)
        cloud.online = false
        try a.write(kind: "note", id: "x", fields: ["text": "v1"])
        try a.write(kind: "note", id: "y", fields: ["text": "other"])
        try a.write(kind: "note", id: "x", fields: ["text": "v2"])
        let pendingWhileOff = a.pendingCount
        cloud.online = true
        clock.advance(10); a.tick()
        try b.pull()
        check("P-retry-coalesce", pendingWhileOff == 2 && cloud.pushCount == 2 && b.row(kind: "note", id: "x")?.fields["text"] == "v2" && b.row(kind: "note", id: "y") != nil,
              "three offline writes leave \(pendingWhileOff) waiting; \(cloud.pushCount) pushes sent; B has v2")
    }
    run("P-pull-offline") {
        let clock = VClock(), cloud = FakeCloudContainer()
        let b = device("iphone-B", cloud, clock)
        cloud.online = false
        var failed = false
        do { try b.pull() } catch { failed = true }
        var offline = false; if case .offline = b.state { offline = true }
        cloud.online = true
        try b.pull()
        check("P-pull-offline", failed && offline && b.statusLine == "In sync", "a pull without network shows offline, then recovers")
    }
}
'''


def sources(src_dir):
    return sorted(os.path.join(src_dir, f) for f in os.listdir(src_dir)
                  if f.endswith(".swift") and f not in (BRIDGE, FEATURE_STUB))


def expiry_s():
    text = open(os.path.join(ROOT, "docs", "CONSTANTS.md"), encoding="utf-8").read()
    m = re.search(r"\|\s*QR_EXPIRY_S\s*\|\s*(\d+)", text)
    if not m:
        raise SystemExit("QR_EXPIRY_S missing from docs/CONSTANTS.md")
    return int(m.group(1))


def static_checks(src_dir):
    """S-no-cloud-call: no iCloud or network API in the folder. S-no-literal-expiry: the expiry number lives only in CONSTANTS."""
    out = {}
    bad_api = re.compile(r"\b(CKContainer|CKDatabase|CKRecord|URLSession|NWConnection)\b|import\s+CloudKit")
    hits = []
    for f in os.listdir(src_dir):
        if f.endswith(".swift"):
            for i, line in enumerate(open(os.path.join(src_dir, f), encoding="utf-8"), 1):
                if bad_api.search(line.split("//")[0]):
                    hits.append("%s:%d" % (f, i))
    out["S-no-cloud-call"] = (not hits, "no iCloud or network call in the folder" if not hits else "found " + ", ".join(hits))
    lit = [f for f in os.listdir(src_dir) if f.endswith(".swift")
           and re.search(r"\b%d\b" % expiry_s(), open(os.path.join(src_dir, f), encoding="utf-8").read())]
    out["S-no-literal-expiry"] = (not lit, "QR_EXPIRY_S is not typed into the folder" if not lit else "literal in " + ", ".join(lit))
    return out


def build_and_run(src_dir, mode, work):
    exe = os.path.join(work, "driver-" + mode)
    drv = os.path.join(work, "main.swift")
    open(drv, "w", encoding="utf-8").write(DRIVER)
    cmd = ["xcrun", "swiftc", "-j", "2", "-O", "-o", exe] + sources(src_dir) + [drv]
    b = subprocess.run(cmd, capture_output=True, text=True)
    if b.returncode != 0:
        return None, "compile failed: " + (b.stderr.strip().splitlines() or ["?"])[-1]
    r = subprocess.run([exe, mode, str(expiry_s())], capture_output=True, text=True)
    res = {}
    for line in r.stdout.splitlines():
        m = re.match(r"CHECK (\S+) (PASS|FAIL) ?(.*)", line)
        if m:
            res[m.group(1)] = (m.group(2) == "PASS", m.group(3))
    if r.returncode != 0 and not res:
        return None, "driver crashed: " + (r.stderr.strip().splitlines() or ["?"])[-1]
    return res, ""


def run_group(ids, mode, src_dir=SRC, with_static=True):
    work = tempfile.mkdtemp(prefix="devicesync-")
    try:
        res, err = build_and_run(src_dir, mode, work)
        res = dict(res or {})
        if with_static:
            res.update(static_checks(src_dir))
        return {i: res.get(i, (None, err or "no result")) for i in ids}
    finally:
        shutil.rmtree(work, ignore_errors=True)


def report(results):
    f = sum(1 for ok, _ in results.values() if ok is False)
    u = sum(1 for ok, _ in results.values() if ok is None)
    for i, (ok, d) in results.items():
        print("%s %s %s" % ("PASS" if ok else ("FAIL" if ok is False else "NOT-JUDGED"), i, d))
    print("%d checks, %d failed, %d not judged" % (len(results), f, u))
    return 1 if f or u else 0


# (mutant name, file, old, new, the check that must go red)
MUTANTS = [
    ("credential guard off", "DeviceSyncEngine.swift", "if let bad = CredentialGuard.offence(in: fields) { throw SyncRefusal.credential(bad) }", "", "F-credential-refused"),
    ("token not consumed", "PairingToken.swift", "live = nil\n        used.insert(hash)", "", "F-token-single-use"),
    ("token never expires", "PairingToken.swift", "if clock() >= current.expiresAtMs { return .failure(.expired) }", "", "F-token-expiry"),
    ("old link stays valid", "PairingToken.swift", "        live = (Self.hash(value), expires)\n", "        if live == nil { live = (Self.hash(value), expires) }\n", "F-token-replaced"),
    ("QR carries the account", "PairingToken.swift", 'public var qrPayload: String { "whoosh-pair:v1:\\(value)" }', 'public var qrPayload: String { "whoosh-pair:v1:\\(value):owner@example.test" }', "F-token-qr-only"),
    ("oldest write wins", "DeviceSyncEngine.swift", "wins = (incoming.updatedMs, incoming.deviceId) > (have.updatedMs, have.deviceId)", "wins = (incoming.updatedMs, incoming.deviceId) < (have.updatedMs, have.deviceId)", "F-lww"),
    ("plaintext payload", "DeviceSyncEngine.swift", "let box = try SyncCrypto.seal(body, kind: row.kind, id: row.id, key: try keys.key())", "let box = body", "F-sealed-payload"),
    ("device-only kinds travel", "DeviceSyncEngine.swift", "guard SyncedKinds.isSynced(kind) else { throw SyncRefusal.localOnly(kind) }", "", "F-local-only"),
    ("record bound to nothing", "SyncCrypto.swift", 'static func aad(kind: String, id: String) -> Data { Data("\\(kind)|\\(id)".utf8) }', "static func aad(kind: String, id: String) -> Data { Data() }", "F-tamper-refused"),
    ("library icon uploads a file", "IconSync.swift", 'fields["icon_id"] = id\n', 'fields["icon_id"] = id\n            try write(kind: "account_icon_file", id: "lib-" + id, fields: ["bytes_b64": ""])\n', "F-library-icon-id-only"),
    ("uploaded image stays home", "IconSync.swift", 'fields["icon_kind"] = "file"', 'fields["icon_kind"] = "library"', "F-icon-upload-travels"),
    ("iCloud call added", "DeviceSyncEngine.swift", "import CryptoKit\n\n/// How long", "import CryptoKit\nimport CloudKit\n\n/// How long", "S-no-cloud-call"),
    ("expiry typed in", "PairingToken.swift", "private let expirySeconds: Int\n", "private let expirySeconds: Int\n    private let typed = 180\n", "S-no-literal-expiry"),
    ("no timer retry", "DeviceSyncEngine.swift", "let due = lastWasFailure && clock() >= retryAtMs", "let due = false", "P-retry-offline"),
    ("failed push drops the queue", "DeviceSyncEngine.swift", "                failures += 1\n", "                failures += 1\n                queue.removeAll()\n", "P-retry-offline"),
    ("flat backoff", "DeviceSyncEngine.swift", "let index = min(max(failures - 1, 0), backoffSeconds.count - 1)", "let index = 0", "P-backoff-steps"),
    ("queue keeps duplicates", "DeviceSyncEngine.swift", '        queue.removeAll { $0 == "\\(kind)|\\(id)" }\n', "", "P-retry-coalesce"),
    ("nothing is pulled", "FakeCloudContainer.swift", "records.values.filter { $0.seq > since }", "records.values.filter { $0.seq > since + 100000 }", "P-mac-to-mac"),
    ("nothing is pulled (settings)", "FakeCloudContainer.swift", "records.values.filter { $0.seq > since }", "records.values.filter { $0.seq > since + 100000 }", "F-settings-sync"),
    ("credential guard off (scan)", "DeviceSyncEngine.swift", "if let bad = CredentialGuard.offence(in: fields) { throw SyncRefusal.credential(bad) }", "", "F-no-credential-scan"),
    ("accounts do not travel", "SyncRecords.swift", '"account", "account_icon_file", "setting"', '"account_icon_file", "setting"', "P-iphone-accounts-settings"),
    ("scan does not consume", "PairingToken.swift", "live = nil\n        used.insert(hash)", "", "P-qr-accounts"),
    ("pull failure hidden", "DeviceSyncEngine.swift", "            throw error\n", "            return 0\n", "P-pull-offline"),
]


def selftest(ids, mode, scope):
    base = run_group(ids, mode)
    red = [i for i, (ok, _) in base.items() if not ok]
    print("control (unmutated): %s" % ("all green" if not red else "RED on " + ", ".join(red)))
    bad = bool(red)
    print("MUTANTS (a check is green only after it has been seen red on its mutant)\n| mutant | file | must go red | seen |\n|---|---|---|---|")
    for name, fname, old, new, target in MUTANTS:
        if target not in ids:
            continue
        tmp = tempfile.mkdtemp(prefix="devicesync-mut-")
        try:
            d = os.path.join(tmp, "src")
            shutil.copytree(SRC, d)
            p = os.path.join(d, fname)
            text = open(p, encoding="utf-8").read()
            if old not in text:
                print("| %s | %s | %s | MUTANT DOES NOT APPLY |" % (name, fname, target)); bad = True; continue
            open(p, "w", encoding="utf-8").write(text.replace(old, new, 1))
            r = run_group([target], mode, d)
            ok = r[target][0]
            seen = ok is False or (ok is None and "compile failed" not in r[target][1] and False)
            print("| %s | %s | %s | %s |" % (name, fname, target, "red" if ok is False else ("NOT RED (%s)" % r[target][1] if ok else "not judged: " + r[target][1])))
            bad = bad or ok is not False
        finally:
            shutil.rmtree(tmp, ignore_errors=True)
    print("selftest: %s" % ("every check seen red on its mutant" if not bad else "FAILED"))
    return 1 if bad else 0


def main():
    if "--selftest" in sys.argv:
        return selftest(FUNCTIONAL_IDS, "functional", "functional")
    return report(run_group(FUNCTIONAL_IDS, "functional"))


if __name__ == "__main__":
    sys.exit(main())
