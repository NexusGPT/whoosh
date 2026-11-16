import Foundation
import Observation

/// The Welcome screen's state: the typed address, the rows, their order. Rows persist (never a secret) so the order
/// survives a relaunch. Every action is a plain method, so the view, the socket's click and the tests call one path.
@MainActor @Observable
public final class WelcomeModel {
    public static let shared = WelcomeModel()
    private static let storeKey = "whoosh.welcome.rows"
    private static let presetKey = "whoosh.welcome.preset"

    public var rows: [WelcomeRow] = []
    public var pasteOpen = false
    public var pasteText = ""
    public var editing: String?
    public var qrOpen = false
    public var preset: String
    public var notice = ""
    /// The field the socket's `type` writes into (a row's editor Name or Signature, or its app-password field): set by the field doors, the way a click focuses a field.
    public var fieldRow: String?
    public var fieldKind = ""
    /// The app passwords typed through the socket, per row (the view keeps its own while a person types).
    public var secrets: [String: String] = [:]
    @ObservationIgnored public var connector: any WelcomeConnector
    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored private var tasks: [String: Task<Void, Never>] = [:]

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        connector = AppModel.shared.host.fixture != nil ? WelcomeFixtureConnector() : WelcomeCoreConnector()
        preset = defaults.string(forKey: Self.presetKey) ?? "Superhuman"
        if let d = defaults.data(forKey: Self.storeKey), let r = try? JSONDecoder().decode([WelcomeRow].self, from: d) { rows = r }
    }

    /// The text field is the app's focused input: the socket's `type` appends to it.
    public var address: String {
        get { AppModel.shared.typed }
        set { AppModel.shared.typed = newValue }
    }
    public var route: WelcomeRoute? { address.isEmpty ? nil : WelcomeRoutes.detect(address) }
    public var healthyCount: Int { rows.filter { $0.phase == .healthy }.count }
    public var refusedCount: Int { rows.filter { $0.phase == .refused }.count }
    public var canOpenInbox: Bool { rows.contains { $0.phase == .healthy || $0.inFlight } }

    // MARK: adding
    @discardableResult
    public func addTyped(secret: String? = nil) -> Bool {
        let a = address.trimmingCharacters(in: .whitespaces)
        defer { address = "" }
        return add(a, secret: secret)
    }

    @discardableResult
    func add(_ raw: String, secret: String?) -> Bool {
        let a = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard WelcomeRoutes.isValid(a) else { notice = a.isEmpty ? "" : Words.text("%@ is not an address.", a); return false }
        guard !rows.contains(where: { $0.address.lowercased() == a.lowercased() }) else { notice = Words.text("%@ is already here.", a); return false }
        let r = WelcomeRoutes.detect(a)
        let row = WelcomeRow(id: "welcome-\(rows.count + 1)-\(a)", address: a, name: a.split(separator: "@").first.map(String.init) ?? a,
                             colour: WelcomeColour.hex(forIndex: rows.count), iconId: nil, signature: "", routeName: r.name,
                             phase: secret != nil ? .connecting : .waiting, status: "", statusLink: "")
        rows.append(row); notice = ""; save()
        if secret != nil { connect(row.id, secret: secret) }
        return true
    }

    /// A pasted list: split on spaces, commas, semicolons and lines; every good address makes a row at once.
    @discardableResult
    public func addPasted() -> Int {
        let parts = pasteText.split(whereSeparator: { " ,;\n\t\r<>".contains($0) }).map(String.init)
        let made = parts.filter { add($0, secret: nil) }.count
        pasteText = ""; pasteOpen = false
        return made
    }

    /// `fixture paste-accounts mac-main-sample` (fixture core only): the rows are the accounts the core already holds, as the core reports them
    /// (id, name, colour, icon, health), in the core's order: a healthy account is a ready row, a failing one a refused row in the core's words.
    public func showCoreAccounts() {
        rows = AppModel.shared.core.accounts().map { a in
            WelcomeRow(id: a.id, address: a.address, name: AccountNames.display(a), colour: a.colour, iconId: a.iconId, signature: "", routeName: WelcomeRoutes.detect(a.address).name,
                       phase: a.health == nil ? .healthy : .refused, status: a.health?.reasonWords ?? "", statusLink: "")
        }
        notice = ""; save()
    }

    /// The end of a drag of one row onto another: the dragged row takes the place of the one it was dropped on.
    @discardableResult
    public func drop(_ id: String, onto other: String) -> Bool {
        guard let i = rows.firstIndex(where: { $0.id == id }), let j = rows.firstIndex(where: { $0.id == other }) else { return false }
        if i != j { move(from: IndexSet(integer: i), to: j > i ? j + 1 : j) }
        return true
    }

    // MARK: connecting
    /// Drive hook for the rig (fakes only): connects every waiting row without a password, so a pasted list shows progress.
    @discardableResult
    public func connectWaiting() -> Int {
        let ids = rows.filter { $0.phase == .waiting }.map(\.id)
        ids.forEach { connect($0, secret: nil) }
        return ids.count
    }
    public func submitSecret(_ id: String, _ secret: String) { connect(id, secret: secret) }

    // MARK: the controls of a row, as plain methods (the view and the socket's doors call these)
    private func row(_ id: String) -> WelcomeRow? { rows.first { $0.id == id } }

    /// Try again on a refused row: it asks for its password again.
    @discardableResult public func tryAgain(_ id: String) -> Bool {
        guard row(id)?.phase == .refused else { return false }
        set(id) { $0.phase = .waiting; $0.status = "" }
        return true
    }
    /// "Sign in with <provider>" on a waiting row of a sheet route.
    @discardableResult public func signInWithSheet(_ id: String) -> Bool {
        guard let r = row(id), r.phase == .waiting, WelcomeRoutes.detect(r.address).usesSheet else { return false }
        submitSecret(id, ""); return true
    }
    /// Connect on a waiting row of a password route: only with words typed in its field, as the button is disabled while the field is empty.
    @discardableResult public func connectTyped(_ id: String) -> Bool {
        guard let r = row(id), r.phase == .waiting, !WelcomeRoutes.detect(r.address).usesSheet, let s = secrets[id], !s.isEmpty else { return false }
        secrets[id] = nil; submitSecret(id, s); return true
    }
    /// A field of a row takes the `type` verb from now on. The editor's fields exist only while the editor is open; the password field while the row waits.
    @discardableResult public func focusField(_ id: String, _ kind: String) -> Bool {
        guard let r = row(id) else { return false }
        switch kind {
        case "name", "signature": guard editing == id else { return false }
        case "secret": guard r.phase == .waiting, !WelcomeRoutes.detect(r.address).usesSheet else { return false }
        default: return false
        }
        fieldRow = id; fieldKind = kind; return true
    }
    /// The words of `type`, appended to the focused field; false when no field holds the focus.
    public func typeIntoField(_ text: String) -> Bool {
        guard let id = fieldRow else { return false }
        switch fieldKind {
        case "name": set(id) { $0.name += text }
        case "signature": set(id) { $0.signature += text }
        case "secret": secrets[id, default: ""] += text
        default: return false
        }
        return true
    }
    @discardableResult public func pickColour(_ id: String, index: Int) -> Bool {
        guard editing == id, WelcomeColour.swatches.indices.contains(index) else { return false }
        set(id) { $0.colour = WelcomeColour.swatches[index] }; return true
    }
    @discardableResult public func pickIcon(_ id: String, icon: String) -> Bool {
        guard editing == id, AccountMarkIcon.libraryIds[icon] != nil else { return false }
        set(id) { $0.iconId = icon }; return true
    }
    @discardableResult public func removeFromEditor(_ id: String) -> Bool {
        guard editing == id else { return false }
        remove(id); editing = nil; return true
    }

    func connect(_ id: String, secret: String?) {
        guard let row = rows.first(where: { $0.id == id }) else { return }
        let route = WelcomeRoutes.detect(row.address)
        tasks[id]?.cancel()
        tasks[id] = Task { [connector] in
            let outcome = await connector.connect(address: row.address, route: route, secret: secret) { phase in self.set(id) { $0.phase = phase; $0.status = "" } }
            await MainActor.run {
                switch outcome {
                case .healthy: self.set(id) { $0.phase = .healthy; $0.status = ""; $0.statusLink = "" }
                case .refused(let w, let l): self.set(id) { $0.phase = .refused; $0.status = w; $0.statusLink = l }
                }
            }
        }
    }

    func set(_ id: String, _ change: (inout WelcomeRow) -> Void) {
        guard let i = rows.firstIndex(where: { $0.id == id }) else { return }
        change(&rows[i]); save()
    }

    // MARK: arranging
    public func move(from: IndexSet, to: Int) {
        var r = rows
        let moving = from.sorted().map { r[$0] }
        for i in from.sorted().reversed() { r.remove(at: i) }
        r.insert(contentsOf: moving, at: min(max(0, to - from.filter { $0 < to }.count), r.count))
        rows = r; save()
    }
    /// Waits for every connection started so far (the tests and the rig's drive hook await it; no view calls it).
    public func settle() async { for t in Array(tasks.values) { await t.value } }
    public func remove(_ id: String) { tasks[id]?.cancel(); rows.removeAll { $0.id == id }; save() }
    public func setPreset(_ p: String) { preset = p; defaults.set(p, forKey: Self.presetKey) }
    public func reset() { tasks.values.forEach { $0.cancel() }; tasks = [:]; rows = []; address = ""; pasteText = ""; notice = ""; qrOpen = false; editing = nil; fieldRow = nil; fieldKind = ""; secrets = [:]; ImportModel.shared.reset(); save() }

    private func save() { if let d = try? JSONEncoder().encode(rows) { defaults.set(d, forKey: Self.storeKey) } }
}
