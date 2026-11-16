import SwiftUI

/// The iPhone's first launch (docs/JOURNEYS.md J4.10): scan the code from the Mac, the accounts arrive, one ask for notifications,
/// then the inbox. Rows, clicks and the `type` door register here, never in a hub.
/// ax ids follow `<surface>.<region>[.<element>...][.<index>]` (checks/welcomephone-check.py).
public enum WelcomePhoneFeature: FeatureModule {
    public static let id = "welcomephone"
    static let surface = "iphone-welcome-accounts"
    public static let surfaces: [String] = [surface]

    static var flow: WelcomePhoneFlow { .shared }

    /// The shell's main slot on the phone: the welcome first, the inbox after it. (The Mac's shell claims this slot earlier on a Mac.)
    public static func view(for slot: SurfaceSlot) -> AnyView? {
        #if os(iOS)
        slot == .macMain ? AnyView(WelcomePhoneRoot()) : nil
        #else
        nil
        #endif
    }

    public static func hasFocus(_ m: AppModel) -> Bool { m.surface == surface && flow.step != .done }

    /// The socket's `type`: a payload as if scanned (a test copy also takes the `fixture:` words). Only the scanning step takes it.
    public static func typeText(_ text: String, _ m: AppModel) -> Bool { hasFocus(m) && flow.typed(text, m) }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "iphone-welcome.step": { _ in
                [["ax": "iphone-welcome.step", "step": flow.step.rawValue, "scans": flow.scans]]
            },
            "iphone-welcome.controls": { _ in
                WelcomeControls.of(flow.step).map { ["ax": $0.ax, "label": $0.label, "enabled": true] }
            },
            "iphone-welcome.fields": { _ in
                WelcomeControls.fields(flow.step).enumerated().map { ["ax": "iphone-welcome.fields.\($0.offset)", "kind": $0.element] }
            },
            "iphone-welcome.refusal": { _ in
                guard flow.step == .refused, let why = flow.refusal else { return [] }
                return [["ax": "iphone-welcome.refusal", "reason": why.rawValue, "text": why.words, "scan_again": true]]
            },
            "iphone-welcome.count": { _ in
                [["ax": "iphone-welcome.count", "arrived": flow.arrived.count, "total": flow.total, "complete": flow.complete]]
            },
            "iphone-welcome.accounts": { _ in
                flow.arrived.enumerated().map { i, a in
                    ["ax": "iphone-welcome.accounts.\(i)", "id": a.id, "name": a.name, "address": a.address, "group": a.group ?? "",
                     "mark_label": a.markLabel, "mark_colour": a.colour, "mark_icon": a.markIcon, "mark_attention": a.needsAttention]
                }
            },
            "iphone-welcome.notify": { _ in
                [["ax": "iphone-welcome.notify", "asked": flow.asked, "answer": flow.notifyAnswer]]
            },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        func on(_ steps: [WelcomeStep], _ act: @escaping (AppModel) -> Void) -> (AppModel) -> Bool {
            { m in guard steps.contains(flow.step) else { return false }; act(m); return true }
        }
        return [
            WelcomeControls.scan.ax: on([.first, .noMac]) { _ in flow.start() },
            WelcomeControls.noMac.ax: on([.first, .refused]) { _ in flow.noMacYet() },
            WelcomeControls.cancel.ax: on([.scanning]) { _ in flow.back() },
            WelcomeControls.scanAgain.ax: on([.refused]) { _ in flow.start() },
            WelcomeControls.next.ax: on([.arriving]) { m in flow.toNotify(m) },
            WelcomeControls.allow.ax: on([.notify]) { m in flow.answerNotify(allow: true, m) },
            WelcomeControls.notNow.ax: on([.notify]) { m in flow.answerNotify(allow: false, m) },
            // The test door: a test copy starts its first launch again (nothing on screen draws it; a real phone refuses it).
            "iphone-welcome.test-restart": { m in
                guard WelcomeTestCopy.on else { return false }
                flow.restartForTest(m)
                return true
            },
        ]
    }
}

/// The phone's first window: the welcome until its last step, then the inbox's own root.
public struct WelcomePhoneRoot: View {
    private let flow = WelcomePhoneFlow.shared
    private let m = AppModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    public init() {}

    /// The welcome shows while no account exists and leaves when one does (PhoneRoot.showsWelcome, tested without a screen). A test copy follows
    /// the flow alone: its fixture core always lists accounts. Reading `m.counts` redraws this when the core's change stream lands one.
    private var welcome: Bool {
        _ = m.counts
        return PhoneRoot.showsWelcome(welcomeDone: flow.step == .done, midArrival: flow.step == .arriving || flow.step == .notify,
                                      accounts: m.core.accounts().count, testCopy: WelcomeTestCopy.on)
    }

    public var body: some View {
        let showing = welcome
        ZStack {
            if showing {
                WelcomeStepHost(flow: flow, m: m)
            } else {
                InboxPhoneRoot().transition(.opacity)
            }
        }
        .animation(reduceMotion ? nil : WhooshMotion.readerSwap, value: showing)
        .animation(reduceMotion ? nil : WhooshMotion.readerSwap, value: flow.step)
        .onAppear { m.surface = showing ? WelcomePhoneFeature.surface : "iphone-inbox" }
    }
}
