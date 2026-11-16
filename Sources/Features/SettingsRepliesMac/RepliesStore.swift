import Foundation
import Observation

/// What Settings › Réponses shows: the core's `reply/...` reads, kept here only between two reads. Every control writes ONE delta 20 command
/// and then reads the core again, so the pane always draws what the core holds (docs/lanes/replies-contract.md). Nothing here waits on a
/// model or the network: a command returns at once and long work (a trial, the voice analysis, a folder's sync) reports by state and event.
@MainActor @Observable
final class RepliesStore {
    static let shared = RepliesStore()

    /// The page shown when the window has not taken the place over yet (RepliesRouter).
    var page: RepliesPage = .auto
    private(set) var uses: [ReplyUseKind: ReplyUse] = [:]
    private(set) var filter: ReplyFilter?
    private(set) var onDemand = defaultOnDemandOptions()
    private(set) var counts: ReplyStepCounts?
    private(set) var sources = RepliesSources.empty
    private(set) var retrieval: RetrievalSettings?
    private(set) var voice: ReplyVoice?
    private(set) var analysis = ReplyVoiceAnalysis.idle
    private(set) var tones: [ReplyTone] = []
    private(set) var prompt: ReplyPrompt?
    private(set) var versions: [ReplyPrompt] = []
    private(set) var variables: [ReplyPromptVariable] = []
    private(set) var notes: [FixedNote] = []
    /// What « Voir ce que le modèle reçoit » shows, and whether it is the saved version or the text being edited.
    var preview: String?
    var previewOfDraft = false
    /// The last trial (« Essayer sur un message »), written by tryOnMessage.
    var trial: ReplyTryReport?
    /// The labels the filter can tick (the owner's labels and the built-in ones, never a view).
    private(set) var labels: [MailLabel] = []
    /// The reason the last write was refused, in the core's words; cleared by the next write that lands.
    var error: String?
    /// Counts refusals, so a switch nudges back to where it was.
    private(set) var refusals = 0
    /// Counts every read, so a socket row can tell a re-read happened.
    private(set) var reads = 0

    /// The one open popover of the pane (a list, the trial, the preview).
    var popover: RepliesPopover?
    /// The live editors of the long texts, by key: kept here so a draft survives leaving the page (RepliesEditorState).
    @ObservationIgnored var editors: [RepliesEditorKey: RepliesEditorState] = [:]
    /// The title of the fixed note being written in the Notes list.
    var newNoteTitle = ""
    /// The sample size picked before « Analyser » (nil: the size of the last run, which the core keeps).
    var voiceSample: UInt32?
    /// The account cards opened in Sources (the first account opens on its own).
    var openAccounts: Set<String> = []
    /// The first account's card opens once, on the first read; a card the owner closed (or the check's click) stays closed on the next read (round 8).
    var openedByDefault = false

    var core: WhooshCore { AppModel.shared.core }

    // MARK: reads

    /// hid-ai round 18: the shipped defaults the model reads (the reply prompt, the retrieval prompt, the AI filter question, the AI card's description)
    /// follow the app's language while the owner has not touched them: the store is given the app language's catalogue text, so what the pane shows is
    /// what the engine sends. A text the owner wrote is never touched (RepliesWords.languageDefault returns nil for it). The prompt's change is a new
    /// version (a version never changes). Written straight to the core, not through `run`: no refusal line, no fault seam, no re-entry into `load`.
    /// Called at the top of every `load`, at launch (SettingsRepliesMacFeature.start) and on a language change.
    func followLanguage() {
        let c = core
        func write(_ command: Command) { _ = try? c.run(command: command) }
        if let r = RepliesRead.retrieval(RepliesRead.object(c.state(path: "reply/retrieval"))) {
            if let t = RepliesWords.languageDefault(r.retrievalPrompt, key: RepliesWords.retrievalPromptDefaultKey) { write(.setRetrievalSetting(change: .prompt(text: t))) }
            if let q = r.aiFilterQuestion, let t = RepliesWords.languageDefault(q, key: RepliesWords.aiFilterDefaultKey) {
                write(.setRetrievalSetting(change: .aiFilterQuestion(question: t)))
            }
        }
        if let p = RepliesRead.object(c.state(path: "reply/prompt/current")).map(RepliesRead.prompt),
           let t = RepliesWords.languageDefault(p.text, key: RepliesWords.promptDefaultKey) {
            write(.saveReplyPromptVersion(promptId: p.promptId, text: t))
        }
        if let f = RepliesRead.filter(RepliesRead.object(c.state(path: "reply/filter"))),
           let t = RepliesWords.languageDefault(f.aiDescription, key: RepliesWords.aiDescriptionDefaultKey) {
            write(.setReplyFilterAiDescription(description: t))
        }
    }

    func load() {
        followLanguage()
        let c = core
        uses[.auto] = RepliesRead.use(RepliesRead.object(c.state(path: "reply/use/auto")), .auto)
        uses[.onDemand] = RepliesRead.use(RepliesRead.object(c.state(path: "reply/use/on_demand")), .onDemand)
        filter = RepliesRead.filter(RepliesRead.object(c.state(path: "reply/filter")))
        if let o = RepliesRead.object(c.state(path: "reply/on_demand")) {
            onDemand = OnDemandOptions(instructionBefore: RepliesRead.flag(o["instruction_before"]), changeAfter: RepliesRead.flag(o["change_after"]))
        }
        counts = RepliesRead.counts(RepliesRead.object(c.state(path: "reply/counts")))
        sources = RepliesRead.sources(RepliesRead.object(c.state(path: "reply/sources")))
        retrieval = RepliesRead.retrieval(RepliesRead.object(c.state(path: "reply/retrieval")))
        if let v = RepliesRead.voice(RepliesRead.object(c.state(path: "reply/voice"))) { voice = v.voice; analysis = v.analysis; tones = v.tones }
        prompt = RepliesRead.object(c.state(path: "reply/prompt/current")).map(RepliesRead.prompt)
        versions = RepliesRead.list(c.state(path: "reply/prompt/versions")).map(RepliesRead.prompt)
        variables = RepliesRead.variables(c.state(path: "reply/prompt/variables"))
        notes = RepliesRead.notes(c.state(path: "reply/notes"))
        labels = labelsWithoutViews(labels: c.labels())
        if !openedByDefault, openAccounts.isEmpty, let first = sources.accounts.first?.accountId { openAccounts = [first]; openedByDefault = true }
        for (k, e) in editors { e.take(saved: savedText(k)) }
        reads += 1
    }

    /// One read of the sources and their sync state (a ReplySourceProgress event names one folder; the read answers for all).
    func reloadSources() { sources = RepliesRead.sources(RepliesRead.object(core.state(path: "reply/sources"))) }


    func use(_ kind: ReplyUseKind) -> ReplyUse? { uses[kind] }

    // MARK: the one door for every write

    /// Runs one command. A refusal (the fixture's armed fault, or the core's own words) leaves the value where it was and says why; either way
    /// the pane then reads the core again. Returns the command's created id ("" when it creates nothing), nil when refused.
    @discardableResult
    func run(_ command: Command) -> String? {
        if let why = FixtureFaults.take(FixtureFaults.op(of: command)) { refuse(why); load(); return nil }
        do {
            let outcome = try core.run(command: command)
            error = nil
            load()
            return outcome.createdId ?? ""
        } catch {
            refuse(Self.words(error))
            load()
            return nil
        }
    }

    private func refuse(_ why: String) { error = why; refusals += 1 }

    static func words(_ error: Error) -> String {
        switch error {
        case CoreError.Action(let failure): return failure.reasonWords
        case CoreError.Invalid(_, let reasonWords): return reasonWords
        case CoreError.NotFound: return RepliesWords.notFound
        case CoreError.Unexpected(let reasonWords): return reasonWords
        default: return RepliesWords.notSaved
        }
    }

    func clearError() { error = nil }
}

/// The popovers a pane opens: one at a time.
enum RepliesPopover: String, Sendable { case trial, fields, aiFilter, tones, variables, notes, versions, preview }
