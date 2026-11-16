import Foundation

/// The frame budget's verdict on the owner's own copy (the hidden test rig cannot measure it: its windows are at
/// alpha 0, so the picture is drawn on the main thread there). When the app runs on the fixture core, every send
/// that ends appends one JSON line: dropped frames, the longest main-thread turn, the picture's source and time.
/// Written off the main thread once the wave is over: nothing is added to the send or to the wave itself.
/// File: ~/Library/Application Support/Whoosh/perf/sendwave.jsonl, or WHOOSH_SENDWAVE_PERF (a test's own file).
/// Read by perf/sendwave-report.py.
@MainActor
enum SendWavePerfLog {
    static var file: URL {
        if let p = SendWaveCenter.env["WHOOSH_SENDWAVE_PERF"] { return URL(fileURLWithPath: p) }
        return ModelsFolder.whooshRoot().appendingPathComponent("perf/sendwave.jsonl")
    }

    static func write(_ r: SendWaveCenter.Run) {
        guard AppModel.shared.host.kind == .fixture || SendWaveCenter.env["WHOOSH_SENDWAVE_PERF"] != nil,
              r.state == .done || r.state == .interrupted else { return }
        #if os(macOS)
        let hidden = HiddenRig.isOn
        #else
        let hidden = false
        #endif
        let row: [String: Any] = [
            "at": ISO8601DateFormatter().string(from: Date()), "kind": r.kind, "pane": r.pane, "state": r.state.rawValue, "reason": r.reason,
            "frames": r.frames, "dropped": r.dropped, "max_frame_ms": r.maxFrameMs, "nominal_frame_ms": r.nominalFrameMs,
            "max_busy_ms": r.maxBusyMs, "over_busy": r.overBusy, "snapshot_source": r.snapshotSource, "snapshot_ms": r.snapshotMs,
            "first_frame_ms": r.firstFrameMs ?? -1, "windows": r.windows, "hidden": hidden,
            "frame_ms_budget": WhooshConstants.FRAME_MS,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: row, options: [.sortedKeys]) else { return }
        let url = file
        Task.detached(priority: .utility) {
            let fm = FileManager.default
            try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            // one whole line in ONE write on an O_APPEND descriptor: two writers (the owner's copy and a rig) cannot interleave inside a line
            // (a seek then a write let them tear one: main76 par-quickreply read a half line)
            let fd = open(url.path, O_WRONLY | O_APPEND | O_CREAT, 0o644)
            guard fd >= 0 else { return }
            defer { close(fd) }
            let line = data + Data([0x0A])
            _ = line.withUnsafeBytes { Darwin.write(fd, $0.baseAddress, line.count) }   // the module name: a bare `write` resolves to this type's own method (trial 23)
        }
    }
}
