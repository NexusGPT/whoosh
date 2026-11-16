import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

/// Rig only (the owner's question "AI filter seeing 0 messages", lead's round 10 of hid-ai): can an on-device LANGUAGE model judge the labels the
/// decision model cannot? `fixture textjudge <cases.tsv> <out.json>` (FixtureEvents.run calls `start` and answers its refusal) runs the cases of
/// checks/textjudge-sanity.py on each text backend this test copy has, and writes the table the check reads. It never runs on the owner's copy
/// (`RigNet.rig`), asks nothing outside this Mac (Apple Intelligence's system model and the MLX model folder are both on this Mac), writes
/// only the file the check named, and changes nothing the owner's path reads: no backend, store or engine is touched, the runner builds its own.
///
/// The cases file, one record per line, tab separated, `\t` and `\n` escaped:
///   label <id> <name> <description>
///   mail  <id> <state text>        (the label flow's own state, as decision/src/adapters/wire.rs `state_text` writes it: from, to, cc, subject,
///                                   list header, a blank line, the body cut at DECISION_BODY_CHARS)
/// The table: {"done": bool, "backends": [{"backend", "available", "words", "forms": {"all": [row], "per-label": [row]}}]}, a row being
/// {"mail", "answers": {label: "yes" | "no"}, "unreadable": [label] , "error", "ms", "guided", "raw"}. An answer the strict reading cannot take is
/// listed in `unreadable` (the check counts it as a failure, never as a "no").
@MainActor
public enum TextJudgeRun {
    private static var running = false

    /// Starts the run, or says in words why it does not (the verb's error). Round 11: nothing stops silently. The table is written at once with
    /// `started` (a path this copy cannot write is the verb's error, not a run that never says anything), after every mail, and with `done` and
    /// the reason on every way the run ends.
    public static func start(cases: String, out: String) -> String? {
        guard RigNet.rig(env: nil) else { return "fixture textjudge runs on a test copy only" }
        guard !running else { return "a text-judge run is already going on this copy" }
        guard TextJudgeRunner.writeTable(["done": false, "started": true], to: out) else {
            return "fixture textjudge: this copy cannot write \(out) (write it under the run folder)"
        }
        running = true
        let installer = ModelInstaller(spec: .qwen25, directory: ModelInstaller.defaultDirectory())
        let runner = TextJudgeRunner(casesPath: cases, outPath: out,
                                     backends: [("apple", AppleIntelligenceBackend()), ("mlx", MlxBackend(installer: installer))])
        Task.detached(priority: .userInitiated) {
            await runner.run()
            await MainActor.run { TextJudgeRun.running = false }
        }
        return nil
    }
}

struct TextJudgeLabel: Sendable { let id: String; let name: String; let description: String }
struct TextJudgeMail: Sendable { let id: String; let state: String }

/// The work, off the main actor. One request per mail (form "all") or per mail and label (form "per-label").
struct TextJudgeRunner: Sendable {
    let casesPath: String
    let outPath: String
    let backends: [(String, any TextBackend)]

    static func unescape(_ s: Substring) -> String { s.replacingOccurrences(of: "\\n", with: "\n").replacingOccurrences(of: "\\t", with: "\t") }

    static func read(_ text: String) -> ([TextJudgeLabel], [TextJudgeMail]) {
        var labels: [TextJudgeLabel] = [], mails: [TextJudgeMail] = []
        for line in text.split(separator: "\n") {
            let f = line.split(separator: "\t", omittingEmptySubsequences: false)
            if f.first == "label", f.count == 4 { labels.append(TextJudgeLabel(id: String(f[1]), name: unescape(f[2]), description: unescape(f[3]))) }
            if f.first == "mail", f.count == 3 { mails.append(TextJudgeMail(id: String(f[1]), state: unescape(f[2]))) }
        }
        return (labels, mails)
    }

    static func allPrompt(_ labels: [TextJudgeLabel], _ mail: TextJudgeMail, guided: Bool) -> String {
        var p = "You sort one email into labels. For each label below, decide whether it applies to this email: yes or no.\n\nLabels:\n"
        for l in labels { p += "- \(l.id) (\(l.name)): \(l.description)\n" }
        p += "\nThe email:\n\(mail.state)\n\n"
        if guided { p += "Give one answer for each label, with the label's id exactly as listed." }
        else {
            p += "Answer with exactly one line per label and nothing else, each line the label's id, a colon and yes or no:\n"
            p += labels.map { "\($0.id): yes" }.joined(separator: "\n")
        }
        return p
    }

    static func onePrompt(_ l: TextJudgeLabel, _ mail: TextJudgeMail, guided: Bool) -> String {
        var p = "Does the label \"\(l.name)\" apply to this email? The label means: \(l.description)\n\nThe email:\n\(mail.state)\n\n"
        p += guided ? "Decide whether the label applies." : "Answer with one word and nothing else: yes or no."
        return p
    }

    /// The strict reading of form "all": every non-empty line is "<label id>: yes|no", each label exactly once. Anything else is unreadable.
    static func parseAll(_ text: String, _ labels: [TextJudgeLabel]) -> (answers: [String: String], unreadable: [String]) {
        var answers: [String: String] = [:], bad = false
        let ids = Set(labels.map(\.id))
        for raw in text.split(separator: "\n") {
            let line = raw.trimmingCharacters(in: .whitespaces)
            if line.isEmpty { continue }
            let parts = line.split(separator: ":", maxSplits: 1).map { $0.trimmingCharacters(in: .whitespaces) }
            guard parts.count == 2, ids.contains(parts[0]), answers[parts[0]] == nil, ["yes", "no"].contains(parts[1].lowercased()) else { bad = true; continue }
            answers[parts[0]] = parts[1].lowercased()
        }
        if bad { return ([:], labels.map(\.id)) }      // one stray line and the answer is not taken at all: never half read
        return (answers, labels.map(\.id).filter { answers[$0] == nil })
    }

    /// The strict reading of form "per-label": the word yes or no, a final full stop allowed. Anything else is nil (unreadable).
    static func parseOne(_ text: String) -> String? {
        var w = text.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if w.hasSuffix(".") { w.removeLast() }
        return ["yes", "no"].contains(w) ? w : nil
    }

    func run() async {
        guard let data = FileManager.default.contents(atPath: casesPath), let text = String(data: data, encoding: .utf8) else {
            write(["done": true, "started": true, "error": "the cases file \(casesPath) cannot be read"]); return
        }
        let (labels, mails) = Self.read(text)
        guard !labels.isEmpty, !mails.isEmpty else {
            write(["done": true, "started": true, "error": "the cases file \(casesPath) holds \(labels.count) labels and \(mails.count) mails"]); return
        }
        var out: [[String: Any]] = []
        for (key, backend) in backends {
            let a = backend.availability()
            var entry: [String: Any] = ["backend": key, "name": backend.name, "available": a.isReady, "words": a.words, "reason": Self.systemReason(key, a)]
            guard a.isReady else { out.append(entry); write(["done": false, "started": true, "backends": out]); continue }
            var forms: [String: [[String: Any]]] = [:]
            for form in ["all", "per-label"] {
                var rows: [[String: Any]] = []
                for m in mails {
                    rows.append(await ask(key, backend, form, labels, m))
                    forms[form] = rows
                    entry["forms"] = forms
                    write(["done": false, "started": true, "backends": out + [entry]])
                }
            }
            out.append(entry)
        }
        write(["done": true, "started": true, "backends": out, "labels": labels.map(\.id), "mails": mails.map(\.id)])
    }

    /// A backend it does not run, in the system's own words: for Apple Intelligence, `SystemLanguageModel.default.availability` as the framework
    /// prints it (word for word); for the others the backend's sentence.
    static func systemReason(_ key: String, _ a: LocalModelAvailability) -> String {
        #if canImport(FoundationModels)
        if key == "apple" {
            if #available(macOS 26.0, iOS 26.0, *) { return String(describing: SystemLanguageModel.default.availability) }
            return "macOS older than 26: FoundationModels is not there"
        }
        #endif
        if key == "apple" { return "this build has no FoundationModels framework" }
        return a.words
    }

    /// One request's time bound (no product meaning: a check bound). A request that does not answer in this time is an error of that row, and
    /// the run goes on: a stuck request can never hold the run (round 11).
    static let requestTimeoutS: Double = 90

    private func ask(_ key: String, _ backend: any TextBackend, _ form: String, _ labels: [TextJudgeLabel], _ m: TextJudgeMail) async -> [String: Any] {
        let t0 = Date()
        var row: [String: Any] = ["mail": m.id]
        var answers: [String: String] = [:], unreadable: [String] = [], raw: [String] = [], errors: [String] = []
        var guided = false
        #if canImport(FoundationModels)
        if key == "apple", #available(macOS 26.0, iOS 26.0, *) {
            guided = true
            do {
                if form == "all" {
                    let prompt = Self.allPrompt(labels, m, guided: true)
                    let pairs = try await textJudgeWithin(Self.requestTimeoutS) {
                        let r = try await LanguageModelSession().respond(to: prompt, generating: TextJudgeAllAnswers.self,
                                                                         options: GenerationOptions(maximumResponseTokens: 240))
                        return r.content.answers.map { TextJudgePair(label: $0.label, applies: $0.applies) }
                    }
                    var seen: [String: String] = [:], bad = false
                    for a in pairs {
                        if seen[a.label] != nil || !labels.contains(where: { $0.id == a.label }) { bad = true }
                        seen[a.label] = a.applies ? "yes" : "no"
                        raw.append("\(a.label): \(a.applies)")
                    }
                    if bad { unreadable = labels.map(\.id) } else { answers = seen; unreadable = labels.map(\.id).filter { seen[$0] == nil } }
                } else {
                    for l in labels {
                        let prompt = Self.onePrompt(l, m, guided: true)
                        let applies = try await textJudgeWithin(Self.requestTimeoutS) {
                            try await LanguageModelSession().respond(to: prompt, generating: TextJudgeOneAnswer.self,
                                                                     options: GenerationOptions(maximumResponseTokens: 16)).content.applies
                        }
                        answers[l.id] = applies ? "yes" : "no"
                        raw.append("\(l.id): \(applies)")
                    }
                }
            } catch {
                errors.append(String(describing: error))
                unreadable = labels.map(\.id).filter { answers[$0] == nil }
            }
            row["answers"] = answers; row["unreadable"] = unreadable; row["guided"] = guided
            row["raw"] = String(raw.joined(separator: " | ").prefix(400)); row["ms"] = Int(Date().timeIntervalSince(t0) * 1000)
            if !errors.isEmpty { row["error"] = errors.joined(separator: "; ") }
            return row
        }
        #endif
        do {
            if form == "all" {
                let prompt = Self.allPrompt(labels, m, guided: false), n = 12 * labels.count
                let text = try await textJudgeWithin(Self.requestTimeoutS) { try await backend.respond(prompt: prompt, maxTokens: n) }
                raw.append(text)
                (answers, unreadable) = Self.parseAll(text, labels)
            } else {
                for l in labels {
                    let prompt = Self.onePrompt(l, m, guided: false)
                    let text = try await textJudgeWithin(Self.requestTimeoutS) { try await backend.respond(prompt: prompt, maxTokens: 4) }
                    raw.append("\(l.id): \(text)")
                    if let w = Self.parseOne(text) { answers[l.id] = w } else { unreadable.append(l.id) }
                }
            }
        } catch {
            errors.append(String(describing: error))
            unreadable = labels.map(\.id).filter { answers[$0] == nil }
        }
        row["answers"] = answers; row["unreadable"] = unreadable; row["guided"] = guided
        row["raw"] = String(raw.joined(separator: " | ").prefix(400)); row["ms"] = Int(Date().timeIntervalSince(t0) * 1000)
        if !errors.isEmpty { row["error"] = errors.joined(separator: "; ") }
        return row
    }

    private func write(_ table: [String: Any]) { _ = Self.writeTable(table, to: outPath) }

    /// The table on disk (atomic). False when it cannot be written: the first write's answer is the verb's error.
    @discardableResult
    static func writeTable(_ table: [String: Any], to path: String) -> Bool {
        guard let d = try? JSONSerialization.data(withJSONObject: table, options: [.sortedKeys]) else { return false }
        do { try d.write(to: URL(fileURLWithPath: path), options: .atomic); return true } catch { return false }
    }
}

/// The continuation of `textJudgeWithin`, resumed once: by the answer or by the time bound, whichever comes first.
final class TextJudgeOnce<T: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var cont: CheckedContinuation<T, Error>?
    init(_ c: CheckedContinuation<T, Error>) { cont = c }
    func resume(_ r: Result<T, Error>) {
        lock.lock(); let c = cont; cont = nil; lock.unlock()
        c?.resume(with: r)
    }
}

struct TextJudgePair: Sendable { let label: String; let applies: Bool }
struct TextJudgeTimeout: Error, CustomStringConvertible {
    let seconds: Double
    var description: String { "no answer within \(Int(seconds)) s" }
}

/// Runs `op` and gives up after `seconds`: the caller gets the answer or a TextJudgeTimeout, whatever `op` does (a request that never returns
/// is left to finish on its own; nothing waits for it).
func textJudgeWithin<T: Sendable>(_ seconds: Double, _ op: @escaping @Sendable () async throws -> T) async throws -> T {
    try await withCheckedThrowingContinuation { (c: CheckedContinuation<T, Error>) in
        let once = TextJudgeOnce(c)
        let work = Task { do { once.resume(.success(try await op())) } catch { once.resume(.failure(error)) } }
        Task {
            try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
            once.resume(.failure(TextJudgeTimeout(seconds: seconds)))
            work.cancel()
        }
    }
}

#if canImport(FoundationModels)
/// Guided generation's answer for form "all": one verdict per label (Apple's structured output; the strict check of ids is the runner's).
@available(macOS 26.0, iOS 26.0, *)
@Generable
struct TextJudgeAllAnswers {
    @Guide(description: "One answer for each label listed, in the order listed")
    var answers: [TextJudgeLabelAnswer]
}

@available(macOS 26.0, iOS 26.0, *)
@Generable
struct TextJudgeLabelAnswer {
    @Guide(description: "The label's id, exactly as listed")
    var label: String
    @Guide(description: "true when the label applies to the email, false when it does not")
    var applies: Bool
}

/// Guided generation's answer for form "per-label".
@available(macOS 26.0, iOS 26.0, *)
@Generable
struct TextJudgeOneAnswer {
    @Guide(description: "true when the label applies to the email, false when it does not")
    var applies: Bool
}
#endif
