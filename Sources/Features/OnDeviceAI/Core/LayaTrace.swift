import Foundation

/// Lane fix-aisort round 5, rig copies only: one JSON line per Laya question in the file `WHOOSH_LAYA_TRACE` names (the hidden check
/// `checks/laya-sanity.py`): the question id and kind, the token count of each part of the sequence before and after its cut, the raw
/// logits, the temperature and where it came from, the probabilities. No text of the question or the mail. The owner's copy writes nothing.
enum LayaTrace {
    static let path: String? = {
        let env = ProcessInfo.processInfo.environment
        guard CorePlace.current(env: env).rig, let p = env["WHOOSH_LAYA_TRACE"], !p.isEmpty else { return nil }
        return p
    }()
    private static let lock = NSLock()
    /// Lane fix-aisort round 7: the broken copy of the per-question cache release in `LayaEngine.answer`, for `checks/laya-sanity.py --mutant`
    /// (`WHOOSH_LAYA_MUTANT=keep-cache`, read on a rig copy only; the owner's copy always releases).
    static let keepCache: Bool = {
        let env = ProcessInfo.processInfo.environment
        return CorePlace.current(env: env).rig && env["WHOOSH_LAYA_MUTANT"] == "keep-cache"
    }()

    static func note(id: String, kind: String, bucket: String, bucketFound: Bool, temperature: Double, logits: [Double], probabilities: [Double],
                     parts: LayaSequence.Parts, maxLen: Int, headMaxLen: Int) {
        guard let path else { return }
        let line: [String: Any] = [
            "id": id, "kind": kind, "bucket": bucket, "temperature": temperature, "temperature_from": bucketFound ? "temperature_by_options" : "temperature",
            "logits": logits, "probabilities": probabilities, "max_len": maxLen, "head_max_len": headMaxLen,
            "tokens": ["head_full": parts.headFull, "head_kept": parts.headKept, "options_full": parts.optionsFull, "options_kept": parts.optionsKept,
                       "state_full": parts.stateFull, "state_kept": parts.stateKept, "total": parts.total],
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: line) else { return }
        lock.lock(); defer { lock.unlock() }
        if !FileManager.default.fileExists(atPath: path) { FileManager.default.createFile(atPath: path, contents: nil) }
        guard let h = FileHandle(forWritingAtPath: path) else { return }
        defer { try? h.close() }
        _ = try? h.seekToEnd()
        try? h.write(contentsOf: data + Data("\n".utf8))
    }
}
