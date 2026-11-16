import Foundation
import Observation

/// The steps of the first launch. One screen each; `done` hands the window to the inbox.
enum WelcomeStep: String, CaseIterable {
    case first, scanning, refused, arriving, notify, noMac, done
}

/// One control a step draws: its ax id and its words. The screen and the `rows` answer both read this table,
/// so a control the check taps is a control the screen draws.
struct WelcomeControl: Equatable {
    let ax: String
    let label: String
}

enum WelcomeControls {
    static let scan = WelcomeControl(ax: "iphone-welcome.scan", label: "Scan the code")
    static let noMac = WelcomeControl(ax: "iphone-welcome.no-mac", label: "I have no Mac yet")
    static let cancel = WelcomeControl(ax: "iphone-welcome.cancel", label: "Back")
    static let scanAgain = WelcomeControl(ax: "iphone-welcome.scan-again", label: "Scan again")
    static let next = WelcomeControl(ax: "iphone-welcome.continue", label: "Continue")
    static let allow = WelcomeControl(ax: "iphone-welcome.allow", label: "Allow notifications")
    static let notNow = WelcomeControl(ax: "iphone-welcome.not-now", label: "Not now")

    static func of(_ step: WelcomeStep) -> [WelcomeControl] {
        switch step {
        case .first: [scan, noMac]
        case .scanning: [cancel]
        case .refused: [scanAgain, noMac]
        case .arriving: [next]
        case .notify: [allow, notNow]
        case .noMac: [scan]
        case .done: []
        }
    }

    /// The text inputs each step draws. None draws one: the phone never asks for a password (docs/JOURNEYS.md question 12).
    static func fields(_ step: WelcomeStep) -> [String] { [] }

    static var all: [WelcomeControl] { WelcomeStep.allCases.flatMap { of($0) } }
}

/// The first launch on the phone: scan the code, the accounts arrive, one ask for notifications, the inbox.
/// Mail data stays in the core; this holds only what the welcome screens own.
@MainActor @Observable
final class WelcomePhoneFlow {
    static let shared = WelcomePhoneFlow()
    private static let doneKey = "whoosh.welcomephone.done"

    private(set) var step: WelcomeStep
    private(set) var refusal: WelcomeRefusal?
    private(set) var arrived: [PhoneAccount] = []
    private(set) var total = 0
    /// "", "allowed", "declined", or "skipped-in-test" (a test copy never asks the system).
    private(set) var notifyAnswer = ""
    private(set) var asked = 0
    private(set) var scans = 0
    @ObservationIgnored private var arrival: Task<Void, Never>?
    @ObservationIgnored private lazy var mac = WelcomeMacSide()

    private init() {
        // A phone that finished the welcome but lost every account (it was removed) shows the welcome again (PhoneRoot.launchIsDone, tested).
        step = PhoneRoot.launchIsDone(flagDone: UserDefaults.standard.bool(forKey: Self.doneKey), accounts: CoreHost.shared.core.accounts().count,
                                      testCopy: WelcomeTestCopy.on) ? .done : .first
    }

    var complete: Bool { total > 0 && arrived.count == total }

    // MARK: moving between steps

    func start() {
        if !WelcomeTestCopy.on && !WelcomeScanner.canScan { return refuse(.noCamera) }
        refusal = nil
        step = .scanning
    }

    func back() { if step == .scanning { step = .first } }
    func noMacYet() { if step == .first || step == .refused { step = .noMac } }

    // MARK: the code

    /// `type` on the scanning step: a test word, or a payload as if scanned. False when this step takes no code.
    func typed(_ text: String, _ m: AppModel) -> Bool {
        guard step == .scanning else { return false }
        if WelcomeTestCopy.on {
            let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
            let word = mac.testWord(t)
            if word.known { if let p = word.payload { scanned(p, m) }; return true }
        }
        scanned(text, m)
        return true
    }

    /// A payload as the camera (or the socket) read it.
    func scanned(_ payload: String, _ m: AppModel) {
        guard step == .scanning else { return }
        scans += 1
        guard WelcomeTestCopy.on else { return refuse(.unreachable) }
        switch mac.redeem(payload) {
        case .paired: arrive(m)
        case .refused(let why): refuse(why)
        }
    }

    func refuse(_ why: WelcomeRefusal) {
        refusal = why
        step = .refused
    }

    // MARK: the accounts

    /// The accounts land one by one with a count. Nothing waits on the animation: `skipArrival` (and Continue) completes it.
    private func arrive(_ m: AppModel) {
        let all = Self.accounts(m)
        arrived = []
        total = all.count
        step = .arriving
        arrival?.cancel()
        if WelcomeTestCopy.on || all.isEmpty { arrived = all; return }   // a test copy lands them at once: the check reads the final list
        arrival = Task { [weak self] in
            for a in all {
                try? await Task.sleep(for: .milliseconds(WelcomeMetrics.arrivalStepMs))
                guard !Task.isCancelled, let self, self.step == .arriving else { return }
                self.arrived.append(a)
            }
        }
    }

    /// The accounts the core lists, in its order, as the phone draws them.
    private static func accounts(_ m: AppModel) -> [PhoneAccount] {
        m.core.accounts().map { a in
            PhoneAccount(id: a.id, name: AccountNames.display(a), address: a.address, colour: a.colour, group: a.group, iconId: a.iconId,   // the imported-mail account reads in the app's language
                         needsAttention: a.health != nil)
        }
    }

    func skipArrival(_ m: AppModel) {
        guard step == .arriving, !complete else { return }
        arrival?.cancel()
        arrived = Self.accounts(m)
    }

    func toNotify(_ m: AppModel) {
        guard step == .arriving else { return }
        skipArrival(m)
        step = .notify
    }

    // MARK: notifications, asked once

    func answerNotify(allow: Bool, _ m: AppModel) {
        guard step == .notify, asked == 0 else { return }
        asked = 1
        if allow {
            Task { [weak self] in
                let answer = await WelcomeNotifications.request()
                self?.notifyAnswer = answer
                self?.finish(m)
            }
        } else {
            notifyAnswer = "declined"
            finish(m)
        }
    }

    /// The test door: the first launch again, as a fresh install would show it. Only a test copy calls it.
    func restartForTest(_ m: AppModel) {
        arrival?.cancel()
        refusal = nil
        arrived = []
        total = 0
        notifyAnswer = ""
        asked = 0
        scans = 0
        step = .first
        m.surface = WelcomePhoneFeature.surface
    }

    private func finish(_ m: AppModel) {
        if !WelcomeTestCopy.on { UserDefaults.standard.set(true, forKey: Self.doneKey) }
        m.surface = "iphone-inbox"
        step = .done
    }
}

/// A test copy: the fixture core, or a launch that names a control socket or the hidden rig. It never asks the system for the camera or
/// for notifications, and it never remembers that the welcome was done (every launch is a first launch).
enum WelcomeTestCopy {
    @MainActor static var on: Bool {
        let env = ProcessInfo.processInfo.environment
        return CoreHost.shared.kind == .fixture || env["WHOOSH_SOCKET_DIR"] != nil || env["WHOOSH_RIG_HIDDEN"] == "1"
    }
}
