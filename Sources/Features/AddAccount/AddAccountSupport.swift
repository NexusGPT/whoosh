import Foundation

/// What the add step needs besides the model: words for a refusal, the validation lines from a report, the core's outcome.
extension AddAccountModel {
    /// A password refused by Microsoft or Google also says what to do: sign in with the person's own app ID, below.
    static func withSignInHint(_ words: String, _ kind: OAuthKind?) -> String {
        kind == nil ? words : words + " " + AddAccountWords.useAppId
    }

    static func firstLink(in s: String) -> String {
        guard let r = s.range(of: "https://") else { return "" }
        return String(s[r.lowerBound...].prefix { !$0.isWhitespace }).trimmingCharacters(in: CharacterSet(charactersIn: ".,;)"))
    }

    /// The two lines of the check and the steps under them, from what the core really did.
    static func lines(from r: ValidationReport) -> [Line] {
        let imap: [ValidationStepKind] = [.imapConnect, .imapTls, .imapLogin, .imapList], smtp: [ValidationStepKind] = [.smtpEhlo, .smtpAuth]
        func line(_ text: String, _ kinds: [ValidationStepKind], needsAll: Bool) -> Line? {
            let s = r.steps.filter { kinds.contains($0.kind) }
            guard !s.isEmpty else { return nil }
            let failed = s.contains { !$0.ok }
            return Line(text: text, state: failed ? .failed : (needsAll && s.count < kinds.count ? .failed : .ok), details: s.map(\.words))
        }
        return [line(AddAccountWords.checkingServer, imap, needsAll: false), line(AddAccountWords.checkingSending, smtp, needsAll: false)].compactMap { $0 }
    }

    /// SEAM(delta 14): when DetectionStep gains the mail-record step, add `case .mx: return "mx"` here and set AddAccountWords.mxStepLanded.
    static func stepId(_ s: DetectionStep) -> String {
        switch s { case .ispdb: return "ispdb"; case .autoconfigUrls: return "autoconfig"; case .srv: return "srv"; case .autodiscover: return "autodiscover"; case .mailRecord: return "mx"; case .guess: return "guess" }
    }
}

struct AddOutcome: Sendable { let report: ValidationReport; let createdId: String?; let failure: String?; var failureCode: String? = nil }

/// Receives the DetectionProgress events of the SyncStatus stream and hands them to the screen on the main actor.
final class DetectionSink: EventSink, @unchecked Sendable {
    private let handler: @MainActor (UInt32, DetectionProgress) -> Void
    init(_ handler: @escaping @MainActor (UInt32, DetectionProgress) -> Void) { self.handler = handler }
    func onEvent(event: CoreEvent) {
        guard case .detectionProgress(_, let number, let progress) = event else { return }
        Task { @MainActor in handler(number, progress) }
    }
}
