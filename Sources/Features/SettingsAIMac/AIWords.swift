import Foundation

/// Every sentence the AI settings pane shows, in one place (docs/DECISION-MODEL.md v2 wording where it exists).
enum AIWords {
    static var title: String { Words.text("AI") }
    static var lead: String { Words.text("What the model decides, and who may act on your mail. Keys stay in the Keychain on this Mac.") }
    static var navProviders: String { Words.text("Providers") }
    static var navAgents: String { Words.text("Agents") }
    static var navDecisions: String { Words.text("Decisions") }
    static var providersLead: String { Words.text("Paste a key. Whoosh asks the provider which models the key opens, then lists them.") }
    static var keyPlaceholder: String { Words.text("Paste an API key") }
    static var connect: String { Words.text("Connect") }
    static var connected: String { Words.text("connected") }
    static var notConnected: String { Words.text("not connected") }
    static var checking: String { Words.text("checking") }
    static var refused: String { Words.text("refused") }
    static var refusedWords: String { Words.text("This key is not valid for the provider.") }
    static var noKeyDoorYet: String { Words.text("Whoosh cannot connect this provider yet.") }
    static var chatgptNote: String { Words.text("Signing in with ChatGPT can stop working at any time.") }
    static var chatgptSignIn: String { Words.text("Sign in with ChatGPT") }
    static var speechTitle: String { Words.text("Speech") }
    static var speechLocal: String { Words.text("On this Mac") }
    static var speechReady: String { Words.text("Ready on this Mac") }
    static var speechRetry: String { Words.text("Retry") }
    static var speechFailed: String { Words.text("The download stopped: the network dropped.") }
    static var agentsLead: String { Words.text("An agent is a model with instructions and the actions you allow. Nothing else.") }
    static var agentsYouMade: String { Words.text("Your agents") }
    static var addKeyAbove: String { Words.text("Add a key above") }
    static var dictationTitle: String { Words.text("Dictation") }
    static var speechToText: String { Words.text("Engine") }
    /// The one line under the engine; `%@` is where the key cap is drawn (the view splits the line there).
    static func holdLine(_ marker: String) -> String { Words.text("Hold %@ in a draft and speak. Whoosh writes what you say.", marker) }
    static var dictNoKey: String { Words.text("No key is set for dictation yet. Set one in Settings › Shortcuts.") }
    static var openShortcuts: String { Words.text("Open Shortcuts") }
    static var speechStateReady: String { Words.text("Ready") }
    static var speechStateDownloading: String { Words.text("Downloading…") }
    static var jevConsentLine: String { Words.text("The text of the mail a label's AI card ticks is sent to Jev to sort it. Nothing else.") }
    static var allow: String { Words.text("Allow") }
    static var allowed2: String { Words.text("Allowed") }
    static var savedNotChecked: String { Words.text("Saved, not checked") }
    /// The ONE place a provider state of the contract (`AiProviderState`) becomes a sentence: the pane never prints the core's raw word.
    static func state(_ s: AiProviderState) -> String {
        switch s {
        case .notConnected: notConnected
        case .checking: checking
        case .connected: connected
        case .refused: refused
        case .notChecked: savedNotChecked
        }
    }
    static var withdraw: String { Words.text("Withdraw") }
    static var test: String { Words.text("Test") }
    static func corrections(_ n: Int) -> String { Words.text("%lld corrections", n) }
    static var makeRuleFromThese: String { Words.text("Make a rule from these") }
    static var refitTheModel: String { Words.text("Refit the model") }
    static func fromSender(_ who: String) -> String { Words.text("from %@", who) }
    static var pasteJevKeyFirst: String { Words.text("Paste the Jev key first.") }
    static var jevNotRefitted: String { Words.text("Jev is not refitted: make a rule from these corrections instead.") }
    static var modelOffNoRefit: String { Words.text("The model is off, so there is nothing to refit: choose Local in Settings, Decisions.") }
    static func refitNeeds(_ need: Int, _ have: Int) -> String { Words.text("A refit needs %lld corrections; you have %lld so far.", need, have) }
    static var refitNotOnThisMac: String { Words.text("Whoosh cannot refit the model on this Mac yet: your corrections are kept for when it can.") }
    static var layaNext: String { Words.text("New mail is sorted as it arrives. Mail already in the inbox is sorted when you press %@ on a label.") }
    /// The score part of a model's facts line (pick M1): "index 9.2".
    static func indexScore(_ score: String) -> String { Words.text("index %@", score) }
    static var layaDownload: String { Words.text("Download model") }   // the button takes the verb; "Download" stays the noun
    static var searchGroup: String { Words.text("Search") }
    static var searchByMeaning: String { Words.text("Search by meaning") }
    static func onThisMacOnce(_ size: String) -> String { Words.text("On this Mac. %@, once.", size) }
    static var layaPrivate: String { Words.text("Private. Nothing leaves this Mac.") }
    static func notOnThisMac(_ size: String) -> String { Words.text("Not on this Mac yet. %@, downloaded once.", size) }
    static func downloadingOf(_ got: String, _ total: String) -> String { Words.text("Downloading %@ of %@", got, total) }
    static var cancel: String { Words.text("Cancel") }
    static var retry: String { Words.text("Retry") }
    static var newAgent: String { Words.text("New agent") }
    static var duplicate: String { Words.text("Duplicate") }
    static var missingProvider: String { Words.text("Connect a provider first: this agent needs one.") }
    static var goToProviders: String { Words.text("Go to Providers") }
    static var allowed: String { Words.text("Allowed") }
    static var decisionsLead: String { Words.text("One page per thing Whoosh decides. Each page is one list: the first entry that answers wins.") }
    static var engine: String { Words.text("Engine") }
    static var decisionModel: String { Words.text("Decision model") }
    static var providerTitle: String { Words.text("Provider") }
    static var filesTitle: String { Words.text("What it files") }
    static var jevKeyTitle: String { Words.text("Jev API key") }
    static var engineLocalShort: String { Words.text("Laya · on this Mac") }
    static var engineLocal: String { Words.text("Local") }
    static var engineLocalLine: String { Words.text("Laya runs on this Mac. Nothing leaves.") }
    /// The provider's caption when another model of the list is in use (SPEC A133): "KnowLine 0.8B runs on this Mac. Nothing leaves."
    static func engineLocalModelLine(_ name: String) -> String { Words.text("%@ runs on this Mac. Nothing leaves.", name) }
    /// Under the model in use while the model the owner asked for downloads (pick M1): "In use until KnowLine 0.8B is ready."
    static func inUseUntil(_ name: String) -> String { Words.text("In use until %@ is ready.", name) }
    /// The last line of the list (pick M1, "Other model"): its name, its fact line, its two buttons, and Remove on an added line.
    static var otherModel: String { Words.text("Other model") }
    static var otherModelFact: String { Words.text("A Hugging Face address of a model Whoosh can run.") }
    static var add: String { Words.text("Add") }
    static var check: String { Words.text("Check") }
    static var remove: String { Words.text("Remove") }
    /// An added model's licence on its facts line: the tag's id as the hub gives it ("apache-2.0"), or these words for "other" and for none.
    static var ownLicence: String { Words.text("own licence, see its page") }
    static var noLicenceStated: String { Words.text("no licence stated") }
    /// Why the model at a typed address is not added: one sentence each, in the amber line under the field.
    static func hubRefusal(_ r: ModelHub.Refusal) -> String {
        switch r {
        case .notAnAddress: Words.text("This is not a Hugging Face address. Write it as owner/model.")
        case .noModel: Words.text("Hugging Face has no model at this address.")
        case .gated: Words.text("This model needs a sign-in on Hugging Face. Whoosh downloads public models only.")
        case .kind(let kind): Words.text("Whoosh cannot run this model: its kind (%@) is not one it knows.", kind)
        case .ownHead: Words.text("Whoosh cannot run this model yet: it reads decisions through its own head.")
        case .missing(let file): Words.text("Whoosh cannot run this model: it has no %@.", file)
        case .tooLarge(let bytes): Words.text("Whoosh cannot run this model: %@ of weights need more memory than this Mac has.", AttachmentSize.words(bytes))
        case .unreachable: Words.text("Could not reach Hugging Face. Connect, then check again.")
        case .already: Words.text("This model is already in the list.")
        }
    }
    /// Round 12: when Apple's on-device model sorts the labels (LabelJudgeRouter, fix-aisort round 8), the Engine row says so.
    static var engineLocalAppleLine: String { Words.text("Apple Intelligence sorts your labels on this Mac; Laya decides the rest. Nothing leaves.") }
    static var engineJev: String { Words.text("Jev") }
    static var engineJevLine: String { Words.text("TypeSafe, with your key in the Keychain.") }
    static var engineOff: String { Words.text("Off") }
    static var engineOffLine: String { Words.text("Every “ask the model” entry is skipped; each list falls through.") }
    static var jevPrivacy: String { Words.text("Sent for each message: from, to, cc, subject, the start of the body, whether it has a list header. Never attachments, never the whole thread.") }
    static var jevKeyPlaceholder: String { Words.text("Paste your Jev key") }
    static var modelPicker: String { Words.text("Model") }
    static var addRule: String { Words.text("Add a rule") }
    static var addModel: String { Words.text("Ask the model here") }
    static var preview: String { Words.text("Preview on my last 20 messages") }
    static var previewLead: String { Words.text("Runs this list as it stands. Writes nothing.") }
    static var decidedBy: String { Words.text("decided by") }
    static var differs: String { Words.text("differs from today") }
    static var apply: String { Words.text("Apply") }
    static var applied: String { Words.text("Applied") }
    static var trust: String { Words.text("trust it from") }
    static var askTheModel: String { Words.text("Ask the model") }
    static var otherwise: String { Words.text("Otherwise") }
    static var proposedChild: String { Words.text("The model proposes a new label") }
    static var accept: String { Words.text("Accept") }
    static var dismiss: String { Words.text("Dismiss") }
    static var pickLabel: String { Words.text("Pick any label") }
    static var interimNote: String { Words.text("Preview data is a sample until the decision engine lands.") }
    static var paletteTitle: String { Words.text("AI settings") }
    static var paletteDecisions: String { Words.text("Decisions") }
    static var untitledAgent: String { Words.text("Untitled agent") }
    static var proposedName: String { Words.text("Tender 2027") }
    static var agentName: String { Words.text("Name") }
    static var save: String { Words.text("Save") }
    static var mayDoNothing: String { Words.text("May do nothing yet. Pick what it may do.") }
    static var ifWord: String { Words.text("If") }
    static func copyOf(_ name: String) -> String { Words.text("%@ copy", name) }
    static func entries(_ n: Int) -> String { Words.text("%lld entries", n) }
    static func settles(_ what: String) -> String { Words.text("Settles %@. The first entry that answers wins.", what) }

    /// AgentPermission in words (contract enum → the card's sentence).
    static func words(_ p: AgentPermission) -> String {
        switch p {
        case .readMail: Words.text("read mail")
        case .search: Words.text("search")
        case .draft: Words.text("draft")
        case .label: Words.text("label")
        case .sendWithConfirmation: Words.text("send with your confirmation")
        }
    }
}
