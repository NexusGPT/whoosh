import Foundation

/// The rows and click doors of the import step, registered by WelcomeMacFeature next to the Welcome rows (ids under `mac-welcome-accounts.import.`).
/// Rows read the model after a fresh read of the core's state, so a read right after `fixture import-progress` shows the new step.
@MainActor enum ImportRows {
    private static let P = "\(WelcomeMacFeature.W).import"
    private static var m: ImportModel { ImportModel.shared }

    static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "\(P).panel": { _ in
                [["ax": "\(P).panel", "showing": m.showing, "from_settings": m.fromSettings, "skipped": m.skipped, "connected": m.connected,
                  "scanned": m.scanned, "fda_presses": m.fdaPresses, "into": m.intoAccount ?? "", "accounts": m.accounts.map(\.id)]]
            },
            "\(P).sources": { _ in
                m.sources.map { s in
                    ["ax": "\(P).source.\(s.kind)", "id": s.id, "kind": s.kind, "name": s.name, "location": s.location, "count": s.count.map { $0 as Any } ?? NSNull(),
                     "imported": s.imported, "access": s.access, "selected": s.id == m.selected, "startable": s.readable]
                }
            },
            "\(P).progress": { _ in
                m.syncRun()
                let rows: [ControlRow] = m.run.map { r in
                    [["ax": "\(P).progress", "run_id": r.runId, "state": r.state, "done": r.done, "total": r.total.map { $0 as Any } ?? NSNull(),
                      "fraction": r.fraction.map { $0 as Any } ?? NSNull(), "current": r.current, "stored": r.stored, "skipped": r.skipped,
                      "line": m.running ? m.progressLine : m.finishLine]]
                } ?? []
                return rows
            },
            "\(P).refusal": { _ in
                let rows: [ControlRow] = m.refusalWords.map { [["ax": "\(P).refusal", "words": $0, "full_disk_access": m.needsFullDiskAccess]] } ?? []
                return rows
            },
        ]
    }

    static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "\(P).scan": { _ in m.reconnect(); m.scanNow(); return true },
            "\(P).start": { _ in m.start(); return m.run != nil },
            "\(P).cancel": { _ in m.cancel(); return m.run?.isOver == true },
            "\(P).skip": { app in m.leave(app) },
            "\(P).done": { app in m.leave(app) },
            "\(P).reset": { _ in m.reset(); return true },
            // Open Full Disk Access (F-k): drawn only while the scan says the access is missing; the press is counted on a test copy (`panel.fda_presses`), never a system window
            "\(P).open-settings": { _ in
                guard m.needsFullDiskAccess else { return false }
                #if os(macOS)
                ImportPicker.openFullDiskAccess()
                return true
                #else
                return false
                #endif
            },
        ]
        for kind in ["apple-mail", "olm", "mbox"] { t["\(P).select.\(kind)"] = { _ in m.selectKind(kind); return m.selectedSource?.kind == kind } }
        // Doors for the file picker (a test never opens the real one): fixture core only, they hand the scan a path as the picker would.
        for (name, path) in [("choose-olm", "~/Documents/Outlook.olm"), ("choose-mbox", "~/Mail/in.mbox"), ("choose-bad", "~/Documents/notes.txt")] {
            t["\(P).\(name)"] = { _ in
                guard CoreHost.shared.kind == .fixture else { return false }
                m.scanNow(picking: path); return true
            }
        }
        return t
    }
}
