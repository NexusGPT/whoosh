import Foundation

/// The reads of the text-bearing parts: the voice and its tones, the reply prompt (current, versions, variables, preview), the fixed notes and a
/// trial run. Same rule as RepliesRead: the contract's own types, defaults where the JSON says nothing.
extension RepliesRead {
    static func voice(_ o: O?) -> (voice: ReplyVoice, analysis: ReplyVoiceAnalysis, tones: [ReplyTone])? {
        guard let o, let v = o["voice"] as? O else { return nil }
        let a = o["analysis"] as? O ?? [:]
        let analysis: ReplyVoiceAnalysis
        switch a["state"] as? String {
        case "running": analysis = .running(read: u32(a["read"]), of: u32(a["of"]))
        case "failed": analysis = .failed(reasonWords: a["reason_words"] as? String ?? "")
        default: analysis = .idle
        }
        let voice = ReplyVoice(toneText: v["tone_text"] as? String ?? "", sampleSize: u32(v["sample_size"]),
                               analysedAtMs: (v["analysed_at_ms"] as? NSNumber)?.int64Value, samePersonExamples: u32(v["same_person_examples"]))
        let tones = (o["tones"] as? [O] ?? []).map { ReplyTone(id: $0["id"] as? String ?? "", name: $0["name"] as? String ?? "", text: $0["text"] as? String ?? "") }
        return (voice, analysis, tones)
    }

    static func prompt(_ o: O) -> ReplyPrompt {
        ReplyPrompt(promptId: o["prompt_id"] as? String ?? "replies", version: u32(o["version"]), text: o["text"] as? String ?? "",
                    createdAtMs: (o["created_at_ms"] as? NSNumber)?.int64Value ?? 0, current: flag(o["current"]))
    }

    static func variables(_ json: String?) -> [ReplyPromptVariable] {
        list(json).map { ReplyPromptVariable(id: $0["id"] as? String ?? "", nameKey: $0["name_key"] as? String ?? "", meaningKey: $0["meaning_key"] as? String ?? "") }
    }

    static func notes(_ json: String?) -> [FixedNote] {
        list(json).map { FixedNote(id: $0["id"] as? String ?? "", title: $0["title"] as? String ?? "", text: $0["text"] as? String ?? "") }
    }

    /// The preview path answers one JSON string: the prompt as the model receives it.
    static func string(_ json: String?) -> String? {
        guard let data = json?.data(using: .utf8) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])) as? String
    }

    static func step(_ o: Any?) -> ReplyStepReport {
        let s = o as? O ?? [:]
        return ReplyStepReport(result: replyStepResultFromKey(key: s["result"] as? String ?? "") ?? .notReached,
                               matched: StoredRules.conditions(s["matched"]), modelNote: s["model_note"] as? String)
    }

    static func trial(_ o: O?) -> ReplyTryReport? {
        guard let o, let r = o["retrieval"] as? O else { return nil }
        let kept = (r["kept"] as? [O] ?? []).compactMap { e -> ReplyExcerpt? in
            guard let src = ref(e["source"]) else { return nil }
            return ReplyExcerpt(chunkId: e["chunk_id"] as? String ?? "", source: src, dateMs: (e["date_ms"] as? NSNumber)?.int64Value,
                                sender: e["sender"] as? String, folder: e["folder"] as? String, subject: e["subject"] as? String, text: e["text"] as? String ?? "")
        }
        return ReplyTryReport(runId: o["run_id"] as? String ?? "", messageId: o["message_id"] as? String ?? "",
                              kind: replyUseKindFromKey(key: o["kind"] as? String ?? "") ?? .auto,
                              retrieval: ReplyRetrievalReport(questions: r["questions"] as? [String] ?? [], found: u32(r["found"]), kept: kept,
                                                              droppedByAiFilter: u32(r["dropped_by_ai_filter"])),
                              entry: step(o["entry"]), ai: step(o["ai"]), exit: step(o["exit"]), draftText: o["draft_text"] as? String)
    }
}
