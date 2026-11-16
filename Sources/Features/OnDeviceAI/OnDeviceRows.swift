import Foundation

/// What the socket's `rows` verb reads from the step: the state in words and numbers, and the last reply.
@MainActor
enum OnDeviceRows {
    static func state(_ s: OnDeviceSetup) -> ControlRow {
        let st = s.state
        var kind = "", received: Int64 = 0, total: Int64 = s.spec.totalBytes
        switch st {
        case .notInstalled: kind = "not_installed"
        case .downloading(let r, let t): kind = "downloading"; received = r; total = t
        case .paused(let r, let t): kind = "paused"; received = r; total = t
        case .verifying: kind = "verifying"; received = total
        case .ready(let b): kind = "ready"; received = b
        case .failed: kind = "failed"
        case .corrupt: kind = "corrupt"
        }
        return ["state": kind, "received": received, "total": total, "percent": total > 0 ? Int(Double(received) / Double(total) * 100) : 0,
                "model": s.spec.name, "size": ModelInstallState.size(s.spec.totalBytes), "licence": s.spec.licence,
                "words": st.words, "availability": s.availabilityWords]
    }

    /// The on-device row of Settings > AI: `state` is ready, downloading or unavailable; `words` is the sentence to show,
    /// `percent` the progress, `reason` the unavailable sentence. The pane draws it; this lane only supplies it.
    static func provider(_ s: OnDeviceSetup) -> ControlRow { s.status.row }

    static func reply(_ s: OnDeviceSetup) -> [ControlRow] {
        switch s.reply {
        case .none: return []
        case .thinking: return [["kind": "thinking", "text": OnDeviceWords.thinking]]
        case .answer(let t): return [["kind": "answer", "text": t]]
        case .refused(let t): return [["kind": "refused", "text": t]]
        }
    }
}
