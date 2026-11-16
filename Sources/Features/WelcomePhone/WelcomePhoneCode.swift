import Foundation

/// What the phone does with a scanned code. The code carries a pairing token only (docs/JOURNEYS.md question 12):
/// no address, no password. `whoosh-pair:v1:<token>` is exactly `PairingToken.qrPayload`.
enum WelcomeCode {
    static let prefix = "whoosh-pair:v1:"

    /// The token inside a scanned payload, nil for anything that is not a Whoosh code.
    static func token(in payload: String) -> String? {
        let t = payload.trimmingCharacters(in: .whitespacesAndNewlines)
        guard t.hasPrefix(prefix) else { return nil }
        let value = String(t.dropFirst(prefix.count))
        return value.isEmpty ? nil : value
    }
}

/// Why a code was refused: the screen says it in words and offers a new scan.
enum WelcomeRefusal: String, Equatable {
    case notOurs = "not-ours"
    case unknown
    case expired
    case alreadyUsed = "already-used"
    case unreachable
    case noCamera = "no-camera"

    var words: String {
        switch self {
        case .notOurs: Words.text("This is not a Whoosh code. Scan the code in Move accounts to iPhone on your Mac.")
        case .unknown: Words.text("This code is not from your Mac. Scan the code in Move accounts to iPhone.")
        case .expired: Words.text("This code expired after %lld minutes. Your Mac shows a new one.", WhooshConstants.QR_EXPIRY_S / 60)
        case .alreadyUsed: Words.text("This code was already used. Your Mac shows a new one.")
        case .unreachable: Words.text("This phone cannot reach your Mac yet. Check that both are online, then scan again.")
        case .noCamera: Words.text("The camera is off for Whoosh. Turn it on in Settings, then scan again.")
        }
    }
}

enum WelcomePhoneOutcome: Equatable {
    case paired
    case refused(WelcomeRefusal)
}

/// The Mac's side of a pairing, as the test copy has it: the same `PairingService` the Mac's code issues from, on the fixture clock
/// (so `clock advance` expires a code). A real scan reaches the real Mac across the relay, which the core does not offer yet
/// (docs/lanes/welcomephone.md "What is not built"); there the answer is `.unreachable`, never a pretend success.
@MainActor
final class WelcomeMacSide {
    private let service: PairingService
    private var issued: String?

    init() {
        let clock = AppModel.shared.host.clock
        service = PairingService(expirySeconds: WhooshConstants.QR_EXPIRY_S, clock: { clock.nowMs() })
    }

    /// The test words the socket verb `type` hands the flow as if the camera had read them. `known` is false for any other text;
    /// `payload` is nil for the word that only issues a code (`fixture:issue`, read later by `fixture:scan-issued`, after a `clock advance`).
    /// The code itself is never printed or kept in a row: only its outcome shows.
    func testWord(_ word: String) -> (known: Bool, payload: String?) {
        switch word {
        case "fixture:valid": return (true, service.issue().qrPayload)
        case "fixture:issue": issued = service.issue().qrPayload; return (true, nil)
        case "fixture:scan-issued": return (true, issued)
        case "fixture:used":
            let t = service.issue()
            _ = service.redeem(t.value)
            return (true, t.qrPayload)
        case "fixture:wrong": return (true, WelcomeCode.prefix + String(repeating: "0", count: 16))
        case "fixture:foreign": return (true, "https://example.invalid/not-a-whoosh-code")
        default: return (false, nil)
        }
    }

    func redeem(_ payload: String) -> WelcomePhoneOutcome {
        guard let value = WelcomeCode.token(in: payload) else { return .refused(.notOurs) }
        switch service.redeem(value) {
        case .success: return .paired
        case .failure(.expired): return .refused(.expired)
        case .failure(.alreadyUsed): return .refused(.alreadyUsed)
        case .failure(.unknown): return .refused(.unknown)
        }
    }
}
