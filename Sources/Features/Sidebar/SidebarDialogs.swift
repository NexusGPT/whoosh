import SwiftUI

/// The sidebar's two native dialogs: the text prompt (new label, rename, new smart folder) and the delete confirmation.
struct SidebarDialogs: ViewModifier {
    @Bindable var state: SidebarState
    let model: AppModel

    func body(content: Content) -> some View {
        content
            .alert(state.prompt?.title ?? "", isPresented: Binding(get: { state.prompt != nil && Self.systemUIAllowed }, set: { if !$0 { state.cancelPrompt() } })) {
                TextField(state.prompt?.hasRule == true ? Words.text("Name") : Words.text("Label name"), text: $state.promptText)
                if state.prompt?.hasRule == true { TextField(Words.text("Rule, in words"), text: $state.promptRule) }
                Button(state.prompt?.confirmTitle ?? Words.text("Create")) { state.confirmPrompt(model: model) }
                Button(Words.text("Cancel"), role: .cancel) { state.cancelPrompt() }
            } message: {
                if let p = state.prompt, case .newLabel(let parent) = p.kind, parent != nil { Text(Words.text("Inside “%@”.", p.name)) }
            }
            .alert(Words.text("Delete %@ “%@”?", state.deleteAsk?.noun ?? "", state.deleteAsk?.name ?? ""),
                   isPresented: Binding(get: { state.deleteAsk != nil && Self.systemUIAllowed }, set: { if !$0 { state.cancelDelete() } })) {
                Button(Words.text("Delete"), role: .destructive) { state.confirmDelete(model: model) }
                Button(Words.text("Cancel"), role: .cancel) { state.cancelDelete() }
            } message: {
                Text(Words.text("Mail in it stays; only the %@ goes.", state.deleteAsk?.noun ?? ""))
            }
            .onChange(of: state.prompt?.title) { _, t in if let t { Self.record(.prompt, t) } }
            .onChange(of: state.deleteAsk?.name) { _, n in if let n { Self.record(.delete, n) } }
    }

    /// On a hidden rig no alert may show (HiddenSystemUI): the prompt and the confirm stay as STATE only, which the socket
    /// reads (`rows mac-main.sidebar.prompt` / `.confirm`) and answers (`click mac-main.sidebar.prompt.confirm`, `.confirm.yes/no`).
    /// Presenting them there handed SwiftUI a refused sheet whose cancel cleared the state before the socket could answer
    /// (main37: par-menu-rename-label, par-menu-delete-asks-first).
    static var systemUIAllowed: Bool {
        #if os(macOS)
        return HiddenRig.systemUIAllowed
        #else
        return true
        #endif
    }

    /// Records the alert that would have shown (`core state system-ui/alert`).
    enum Ask: String { case prompt = "sidebar-prompt", delete = "sidebar-delete" }
    static func record(_ ask: Ask, _ name: String) {
        #if os(macOS)
        HiddenRig.allowSystemUI("alert", ask.rawValue + " " + name)
        #endif
    }
}
