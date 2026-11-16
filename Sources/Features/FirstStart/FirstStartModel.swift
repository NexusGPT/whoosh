import Foundation
import Observation

/// The first-start flow's state (J9): which of the 10 steps shows, whether the veil is up, and the one persisted fact, that the flow
/// has been seen (`whoosh.firstStart.finished`, the app's own UserDefaults; the rig copy has its own identity, so a check can
/// never set or read the owner's). Every action is one plain method: the buttons, the keys and the socket's clicks call the same one.
///
/// First-launch rule: the flow opens when no account exists AND the flag is unset (`shouldShow`). Finish, and Skip at step 6, both
/// set the flag: a second launch never shows it. The core's `first_start` row (DATA-MAP first-start-state) follows with its contract.
@MainActor @Observable
public final class FirstStartModel {
    public static let shared = FirstStartModel()
    static let finishedKey = "whoosh.firstStart.finished"
    static let stepCount = 10
    /// 6 the account, 7 the AI (what the AI may use: the models and the key), 8 the Réponses choices (they write to the account's folders and need the AI), 9 the import, 10 the lifted veil
    static let addStep = 6, aiStep = 7, repliesStep = 8, importStep = 9, lastStep = 10

    public private(set) var active = false
    public private(set) var step = 1
    /// the step the card came from, for the dots' slide and the card's morph
    public private(set) var previousStep = 1
    /// Skip at step 6: the veil lifts on an empty inbox with an Add account button (owner card 2026-11-09 11:43); steps 7 to 10 are skipped
    public private(set) var skipped = false
    /// the veil is up unless this is true (step 10, the skip state)
    public private(set) var lifted = false
    /// the loops of a step run from here, so every step starts its 8 s at 0
    public private(set) var stepStart = Date()
    /// how many accounts the core holds, read at each change, never polled
    public private(set) var accountCount = 0
    /// each Finish plays the ring once; the ring's own view clears it
    public private(set) var ringId = 0
    var ringShowing = false
    /// the rig's Reduced Motion switch (`fixture first-start reduce on|off`); the real one is the system's
    var rigReduceMotion = false
    private var evaluated = false

    // the try-its
    var tryText = ""
    /// the main window's size, written by the overlay, so the rows can name the anchors the card is at
    var windowSize = CGSize(width: 1280, height: 820)
    var keys = FirstStartKeyTry()

    public var finished: Bool { UserDefaults.standard.bool(forKey: Self.finishedKey) }
    /// the overlay draws while the flow runs, or while the step-8 ring is still crossing
    var visible: Bool { active || ringShowing }
    var frozen: Bool { rigReduceMotion || FirstStartLook.lowPower || SendWaveCenter.reduceMotionOverride }

    private init() {}

    /// The rule, in one place: no account and the flow never finished.
    nonisolated static func shouldShow(accounts: Int, finished: Bool) -> Bool { accounts == 0 && !finished }

    // MARK: launch
    /// Once per launch, from the first window's appearance: opens the flow when the rule says so.
    func evaluateLaunch() {
        guard !evaluated else { return }
        evaluated = true
        relaunch()
    }

    /// The launch rule again (the rig's "relaunch" door): shows the flow only for a first launch.
    @discardableResult
    func relaunch() -> Bool {
        refreshAccounts()
        let show = Self.shouldShow(accounts: accountCount, finished: finished)
        if show { open() } else { active = false; skipped = false; lifted = false }
        return show
    }

    func open() {
        active = true; step = 1; previousStep = 1; skipped = false; lifted = false; stepStart = Date(); shownAgain = false
        addedAtOpen = AddAccountModel.shared.added.count
        refreshAccounts()   // the count the Continue control of the add-account step is drawn by: read at every opening, not only at launch (sweep-firststart on main170: Continue drew enabled with no account, and its press did nothing)
        countAtOpen = accountCount
        tryText = ""; keys = FirstStartKeyTry()
        RepliesStepModel.shared.reset(); AISetupStep.shared.reset()
        AppModel.shared.surface = "mac-main"
    }

    /// The door of an installation that already has an account (Settings, General and the palette; owner, 16 Nov 2026: "we need to have the quickstart of
    /// whoosh live"): the ten steps again from step 1, over the main window. A launch opens the flow only with no account, so without this door an owner who
    /// has an account can never see it. The flag keeps its value: a later launch still never opens the flow by itself.
    @discardableResult
    func replay() -> Bool {
        open()
        shownAgain = true
        WindowBridge.giveMainTheKeyboard()   // the door may be in the Settings window: the flow's keys are the main window's
        return true
    }
    /// The flow was opened by the door, not by a launch: Skip at the account step then goes on when an account exists (no empty inbox to open).
    private(set) var shownAgain = false

    /// `fixture first-start on|off`: opens at step 1 with the flag cleared, or closes with nothing left over.
    func rig(on: Bool) {
        if on { UserDefaults.standard.removeObject(forKey: Self.finishedKey); open() } else { active = false; skipped = false; lifted = false; ringShowing = false }
    }

    /// The accounts the flow counts: the core's list, or, while the core still lists none, the ones the add card saved since the flow opened
    /// (the fixture core under `fixture accounts none` keeps listing none after an add; main54 stayed on step 6 for it).
    func refreshAccounts() {
        let core = CoreHost.shared.core.accounts().count
        accountCount = core > 0 ? core : max(0, AddAccountModel.shared.added.count - addedAtOpen)
    }
    /// how many accounts the add card already held when the flow opened (its list outlives the flow)
    private var addedAtOpen = 0
    /// how many accounts the flow counted when it opened: 0 on a first launch, the installation's own count when the flow is shown again
    private var countAtOpen = 0

    // MARK: moves
    private func go(_ to: Int) {
        guard to >= 1, to <= Self.lastStep else { return }
        previousStep = step; step = to; stepStart = Date()
        if to == 2 { tryText = "" }
        if to == 3 { keys = FirstStartKeyTry() }
    }

    /// Continue ↩ (Finish ↩ at step 9). Step 6 goes on only once an account exists; step 7 is the AI; step 8 writes the three reply choices; false means the press did nothing.
    @discardableResult
    func next() -> Bool {
        guard active, !skipped else { return false }
        switch step {
        case 1...5: go(step + 1); return true
        case Self.addStep:
            refreshAccounts()
            guard accountCount > 0 else { return false }
            go(Self.aiStep); return true
        case Self.aiStep: go(Self.repliesStep); return true   // Continue is always on: what is not set keeps its warn mark
        case Self.repliesStep:
            RepliesStepModel.shared.apply()
            go(Self.importStep); return true
        case Self.importStep: return finish()
        default: return false
        }
    }

    /// Back ⌘[: one step back; none at step 1; the skip state has nothing to go back to.
    @discardableResult
    func back() -> Bool {
        guard active, !skipped, step > 1, step <= Self.importStep else { return false }
        go(step - 1); return true
    }

    /// Skip esc: at steps 1 to 5 it jumps to step 6; at step 6 it opens the empty inbox with Add account (the 11:43 rule); at 7 it goes on and writes
    /// nothing (the reply settings keep their defaults: auto off); at 8 it ends.
    @discardableResult
    func skip() -> Bool {
        guard active, !skipped else { return false }
        switch step {
        case 1...5: go(Self.addStep); return true
        case Self.addStep:
            if shownAgain {   // the flow shown again on an installation with an account: there is no empty inbox to open
                refreshAccounts()
                if accountCount > 0 { go(Self.aiStep); return true }
            }
            return skipToEmptyInbox()
        case Self.aiStep: go(Self.repliesStep); return true   // Skip leaves the marks on
        case Self.repliesStep: go(Self.importStep); return true
        case Self.importStep: return finish()
        default: return false
        }
    }

    /// Step 10: the veil lifts, one ring crosses, the panes are the inbox; the flag is set and the flow never shows again.
    @discardableResult
    func finish() -> Bool {
        if step == Self.importStep { ImportModel.shared.leave(AppModel.shared) }   // Skip and Finish share the panel's own way out
        go(Self.lastStep)
        lifted = true; active = false
        UserDefaults.standard.set(true, forKey: Self.finishedKey)
        ringId += 1; ringShowing = true
        AppModel.shared.refresh()
        return true
    }

    private func skipToEmptyInbox() -> Bool {
        skipped = true; lifted = true
        UserDefaults.standard.set(true, forKey: Self.finishedKey)
        return true
    }

    /// The skip state's Add account ↩: the add-account sheet over the main window (AddAccount's own door).
    @discardableResult
    func addAccountFromEmptyInbox() -> Bool {
        guard active, skipped else { return false }
        AppModel.shared.surface = AddAccountWords.surface
        return true
    }

    /// An account may have been added (the add card's list grew, or the rig added one): at step 6 the flow goes on; in the skip state it ends.
    func accountsMayHaveChanged() {
        refreshAccounts()
        // ADDED means more accounts than when the flow opened. With "> 0" the flow shown again on an installation that has accounts left step 6 at the
        // first click in the address field (main204, probe-j912: step 7 after `click mac-add-account.address`).
        guard active, accountCount > countAtOpen else { return }
        if skipped { active = false; skipped = false; lifted = false }
        else if step == Self.addStep { go(Self.aiStep) }
    }

    func ringDone(_ id: Int) { if id == ringId { ringShowing = false } }

    // MARK: the try-its
    /// Step 2: a word typed into the stage's search field (the real keyboard and the socket's `type` both arrive here).
    func tryType(_ text: String) -> Bool {
        guard active, !skipped, step == 2 else { return false }
        tryText += text
        return true
    }

    func tryBackspace() { if !tryText.isEmpty { tryText.removeLast() } }

    /// Step 3: J, K, E or H on the sample rows. Every other key does nothing at the inbox behind the veil: it is swallowed.
    func tryKey(_ key: String) -> Bool {
        guard active else { return false }
        guard step == 3, !skipped else { return true }
        keys.press(key)
        if keys.rows.isEmpty { returnRowsSoon() }
        return true
    }

    /// The three rows come back a beat after the last one left, so the keys can be tried again.
    private func returnRowsSoon() {
        let tick = keys.tick
        Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(900))
            guard let self, self.keys.tick == tick, self.keys.rows.isEmpty else { return }
            self.keys.rows = FirstStartKeyTry.fresh; self.keys.focus = 0; self.keys.last = ""
        }
    }
}

/// Step 3's real-row try-it (2a): three sample rows, a focus, and what the last key did. A row leaves on E (done) or H (remind);
/// when all have left they return, so the user can press again.
struct FirstStartKeyTry: Equatable {
    var rows: [String] = FirstStartKeyTry.fresh
    var focus = 0
    var started = false
    var last = ""
    var lastKey = ""
    var lastRow = ""
    var tick = 0
    static let fresh = ["m1", "m2", "m3"]

    mutating func press(_ key: String) {
        started = true; tick += 1
        let k = key.uppercased()
        lastKey = k
        if rows.isEmpty { rows = Self.fresh; focus = 0 }
        switch k {
        case "J": focus = min(rows.count - 1, focus + 1); last = "move"
        case "K": focus = max(0, focus - 1); last = "move"
        case "E", "H":
            lastRow = rows[focus]
            rows.remove(at: focus)
            last = k == "E" ? "done" : "remind"
            focus = min(focus, max(0, rows.count - 1))
        default: break
        }
    }
}
