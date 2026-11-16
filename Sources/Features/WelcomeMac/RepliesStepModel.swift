import Foundation
import Observation

/// Lets the core cross into the one background task of an apply (the core is thread-safe by contract; ImportModel does the same).
fileprivate final class RepliesCoreBox: @unchecked Sendable { let core: WhooshCore; init(_ core: WhooshCore) { self.core = core } }

/// The three choices of the Réponses step (first start, step 7). `raw` is the id a door and a row spell.
enum RepliesDraftChoice: String, CaseIterable, Sendable { case onDemand = "on-demand", auto }
enum RepliesSearchChoice: String, CaseIterable, Sendable { case sentAndInbox = "sent-inbox", inbox, sent }
enum RepliesVoiceChoice: Int, CaseIterable, Sendable { case last100 = 100, last300 = 300, last1000 = 1000 }

/// One write the step makes, a delta 20 command (docs/lanes/replies-contract.md): `SetReplyUseEnabled`, `SetReplySourceEnabled`, `RunVoiceAnalysis`.
enum RepliesWrite: Equatable, Sendable {
    case use(auto: Bool)
    case source(account: String, folder: String, enabled: Bool)
    case voice(sample: UInt32)

    var command: Command {
        switch self {
        case .use(let auto): return .setReplyUseEnabled(kind: .auto, enabled: auto)
        case .source(let account, let folder, let enabled): return .setReplySourceEnabled(source: .mailFolder(accountId: account, folder: folder), enabled: enabled)
        case .voice(let sample): return .runVoiceAnalysis(sampleSize: sample)
        }
    }
    /// the command's contract name, for the rows
    var name: String {
        switch self { case .use: return "SetReplyUseEnabled"; case .source: return "SetReplySourceEnabled"; case .voice: return "RunVoiceAnalysis" }
    }
}

/// The choices, and what Continue writes. Skip never calls `apply`: the core keeps its defaults (auto off, on demand on, the folders as they are).
/// No view waits on a command: the real core's three commands only record the choice and start background work (its sync, its analysis);
/// they run off the main thread there. The fixture core answers at once, so the rig's doors and Continue run them in place.
@MainActor @Observable
public final class RepliesStepModel {
    public static let shared = RepliesStepModel()

    var draft: RepliesDraftChoice = .onDemand
    var search: RepliesSearchChoice = .sentAndInbox
    var voice: RepliesVoiceChoice = .last100
    /// the writes of the last apply, by command name; empty until Continue ran it
    private(set) var written: [RepliesWrite] = []
    private(set) var applied = false
    /// the first refusal of the last apply, in words (empty when none)
    private(set) var refusal = ""

    private init() {}

    /// the defaults again: a flow that opens starts from them
    func reset() { draft = .onDemand; search = .sentAndInbox; voice = .last100; written = []; applied = false; refusal = "" }

    /// Continue ↩ at the step: writes the three choices through the contract's commands.
    func apply() {
        let core = CoreHost.shared.core
        let plan = Self.plan(draft: draft, search: search, voice: voice, folders: RepliesState.mailFolders(core))
        written = plan; applied = true; refusal = ""
        if CoreHost.shared.kind == .fixture { refusal = RepliesStepModel.perform(plan, RepliesCoreBox(core)).first ?? ""; return }
        let box = RepliesCoreBox(core)
        Task { [self] in
            let failed = await Task.detached { RepliesStepModel.perform(plan, box) }.value
            refusal = failed.first ?? ""
        }
    }

    /// The writes, in order: the auto switch, each account's Inbox and Sent folder, the voice analysis. A folder the choice leaves out is switched off.
    nonisolated static func plan(draft: RepliesDraftChoice, search: RepliesSearchChoice, voice: RepliesVoiceChoice, folders: [RepliesFolder]) -> [RepliesWrite] {
        var out: [RepliesWrite] = [.use(auto: draft == .auto)]
        for f in folders {
            switch RepliesFolderRole.of(f.folder) {
            case .inbox: out.append(.source(account: f.account, folder: f.folder, enabled: search != .sent))
            case .sent: out.append(.source(account: f.account, folder: f.folder, enabled: search != .inbox))
            case nil: continue
            }
        }
        out.append(.voice(sample: UInt32(voice.rawValue)))
        return out
    }

    fileprivate nonisolated static func perform(_ plan: [RepliesWrite], _ box: RepliesCoreBox) -> [String] {
        var failed: [String] = []
        for w in plan {
            do { _ = try box.core.run(command: w.command) } catch { failed.append(ImportModel.refusalOf(error).1) }
        }
        return failed
    }
}

/// A mail folder as `reply/sources` lists it.
struct RepliesFolder: Equatable, Sendable { let account: String; let folder: String }

/// Which well-known folder a server's folder name stands for (the contract serves the server's own name, never a role).
enum RepliesFolderRole {
    case inbox, sent
    private static let sentNames: Set<String> = ["sent", "sent items", "sent mail", "sent messages", "envoyés", "envoyes", "éléments envoyés", "elements envoyes",
                                                  "gesendet", "gesendete objekte", "gesendete elemente", "verzonden", "verzonden items", "enviados", "enviadas", "inviata", "posta inviata", "skickat"]
    /// the folder's own name (after the last "/" or "."), compared without case
    static func of(_ folder: String) -> RepliesFolderRole? {
        let last = folder.split(whereSeparator: { $0 == "/" }).last.map(String.init) ?? folder
        let name = last.lowercased()
        if name == "inbox" { return .inbox }
        return sentNames.contains(name) ? .sent : nil
    }
}

/// Reads of `reply/sources` (the contract's JSON, docs/lanes/replies-contract.md): the mail folders of every account.
enum RepliesState {
    static func mailFolders(_ core: WhooshCore) -> [RepliesFolder] {
        guard let text = core.state(path: "reply/sources"), let data = text.data(using: .utf8),
              let root = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any], let accounts = root["accounts"] as? [[String: Any]] else { return [] }
        return accounts.flatMap { a -> [RepliesFolder] in
            let id = a["account_id"] as? String ?? ""
            return (a["folders"] as? [[String: Any]] ?? []).compactMap { f in
                guard let s = f["source"] as? [String: Any], s["kind"] as? String == "mail_folder", let name = s["folder"] as? String else { return nil }
                return RepliesFolder(account: s["account_id"] as? String ?? id, folder: name)
            }
        }
    }
}
