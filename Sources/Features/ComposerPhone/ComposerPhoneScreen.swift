import SwiftUI
import UniformTypeIdentifiers

/// The iPhone composer (layout mail-26, style glass-light): Apple Mail 26's compose sheet for this role inside the inbox's frame.
/// Paper ground; glass only on what floats (Cancel, Send, the bar above the keyboard, the undo capsule). Loading, empty and error each
/// have their line: a refusal reads under the header (the core's own words), the speech model's state sits in the dictate pill.
/// The message is `ComposerModel.shared` (drafts are saved first, then sent, through the core); this view only draws it.
public struct ComposerPhoneScreen: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let m = AppModel.shared
    private let state = ComposerPhoneState.shared
    @Bindable private var cm = ComposerModel.shared
    @FocusState private var focused: ComposerPhoneState.Field?

    // Private number: how often the open send asks the core whether its undo window has closed (the Mac composer asks at the same pace).
    private static let reconcileMs = 200

    public init() {}

    public var body: some View {
        VStack(spacing: 0) {
            ComposerPhoneTop(state: state, onCancel: { state.close(m) }, onSend: { _ = state.send(m) })
                .padding(.top, WhooshSpacing.s8)
            fields
            line
            editor
            quoted
        }
        .background(PhoneGround.list.ignoresSafeArea())
        .safeAreaInset(edge: .bottom, spacing: WhooshSpacing.s8) { floating }
        .sheet(isPresented: Binding(get: { state.fromOpen }, set: { state.fromOpen = $0 })) {
            ComposerPhoneFromList(state: state).presentationDetents([.medium]).presentationDragIndicator(.visible)
        }
        .fileImporter(isPresented: Binding(get: { state.attaching }, set: { state.attaching = $0 }), allowedContentTypes: [.item], allowsMultipleSelection: true) { r in
            guard case .success(let urls) = r else { return }
            for u in urls {
                let scoped = u.startAccessingSecurityScopedResource()
                _ = cm.attach(u)
                if scoped { u.stopAccessingSecurityScopedResource() }
            }
        }
        .onAppear { state.appeared(m) }
        .onChange(of: focused) { _, f in if f != nil || state.focus == nil { state.focus = f } }
        .onChange(of: state.focus) { _, f in if let f, f != .body { focused = f } }
        .task(id: cm.outboxId) { await watchWindow() }
        .task(id: state.isSent) { await closeAfterSent() }
    }

    // MARK: header grid

    private var fields: some View {
        VStack(spacing: 0) {
            ComposerPhoneRecipients(kind: .to, model: cm, focus: $focused).overlay(alignment: .trailing) { ccButton }
            completions
            if cm.showCc { ComposerPhoneRecipients(kind: .cc, model: cm, focus: $focused).transition(.opacity) }
            if cm.showBcc { ComposerPhoneRecipients(kind: .bcc, model: cm, focus: $focused).transition(.opacity) }
            ComposerPhoneFrom(state: state)
            ComposerPhoneRow(label: ComposerWords.subject) {
                TextField("", text: $cm.subject).focused($focused, equals: .subject).phoneText(PhoneType.body)
                    .submitLabel(.next).accessibilityLabel(ComposerWords.subject).accessibilityIdentifier(ComposerPhoneIds.subject)
            }
        }
        .disabled(cm.isCounting)
        .animation(reduceMotion ? nil : WhooshMotion.unfold, value: cm.showCc)
        .accessibilityIdentifier(ComposerPhoneIds.header)
    }

    private var ccButton: some View {
        Button {
            let on = !(cm.showCc || cm.showBcc)
            cm.showCc = on; cm.showBcc = on
        } label: {
            Image(systemName: "chevron.down").font(.system(size: PhoneType.footnote.size, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
                .rotationEffect(.degrees(cm.showCc || cm.showBcc ? 180 : 0))
                .frame(width: PhoneMetrics.roundButton, height: PhoneMetrics.roundButton).contentShape(Rectangle())
        }
        .buttonStyle(.plain).padding(.trailing, WhooshSpacing.s4)
        .accessibilityLabel(cm.showCc || cm.showBcc ? Words.text("Hide Cc and Bcc") : Words.text("Show Cc and Bcc"))
        .accessibilityIdentifier(ComposerPhoneIds.ccToggle)
    }

    /// The people the typed letters match (the person being answered, a contact the core has): one tap makes the chip.
    @ViewBuilder private var completions: some View {
        let found = Array(cm.completions.prefix(3))
        if !found.isEmpty {
            VStack(spacing: 0) {
                ForEach(Array(found.enumerated()), id: \.element.id) { i, s in
                    Button { cm.to.append(s.address); cm.toInput = "" } label: {
                        HStack(spacing: WhooshSpacing.s8) {
                            Text(s.name).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label)
                            Text(s.address).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                            Spacer(minLength: 0)
                        }
                        .padding(.horizontal, PhoneMetrics.gutter).frame(minHeight: PhoneMetrics.roundButton).contentShape(Rectangle())
                    }
                    .buttonStyle(.plain).accessibilityIdentifier(ComposerPhoneIds.completion(i))
                }
            }
            .background(PhoneGround.card)
        }
    }

    /// The core's own words for a refusal or a hint ("Add a recipient and some text first."), under the header.
    @ViewBuilder private var line: some View {
        if !cm.line.isEmpty {
            Text(cm.line).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, PhoneMetrics.gutter).padding(.top, WhooshSpacing.s8)
                .accessibilityIdentifier(ComposerPhoneIds.line)
        }
    }

    // MARK: body

    private var editor: some View {
        ZStack(alignment: .topLeading) {
            RichTextEditor(session: cm.session)
                .accessibilityIdentifier(ComposerPhoneIds.body)
                .opacity(cm.isCounting ? 0.5 : 1).allowsHitTesting(!cm.isCounting)
            if cm.session.document.isEmpty {
                Text(ComposerWords.bodyPlaceholder).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.tertiaryLabel)
                    .padding(.horizontal, PhoneMetrics.gutter + WhooshSpacing.s4).padding(.top, WhooshSpacing.s12).allowsHitTesting(false)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    /// A reply carries the message it answers, folded; a tap shows it.
    @ViewBuilder private var quoted: some View {
        if !cm.quoted.isEmpty {
            VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
                Button { withAnimation(reduceMotion ? nil : WhooshMotion.unfold) { state.quotedOpen.toggle() } } label: {
                    HStack(spacing: WhooshSpacing.s6) {
                        Image(systemName: "ellipsis").font(.system(size: PhoneType.footnote.size, weight: .semibold))
                        Text(state.quotedOpen ? "Hide quoted text" : "Show quoted text").phoneText(PhoneType.subhead)
                    }
                    .foregroundStyle(WhooshPalette.secondaryLabel).frame(minHeight: PhoneMetrics.roundButton).contentShape(Rectangle())
                }
                .buttonStyle(.plain).accessibilityIdentifier(ComposerPhoneIds.quotedToggle)
                if state.quotedOpen {
                    ForEach(cm.quoted) { q in
                        VStack(alignment: .leading, spacing: 0) {
                            Text("\(q.from) · \(q.time)").phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                            Text(q.text).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(8)
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, PhoneMetrics.gutter)
            .accessibilityIdentifier(ComposerPhoneIds.quoted)
        }
    }

    // MARK: floating

    private var floating: some View {
        VStack(spacing: WhooshSpacing.s12) {
            if cm.isCounting || state.isSent {
                ComposerPhoneUndo(state: state) { _ = state.undoSend(m) }
                    .transition(reduceMotion ? .opacity : .offset(y: WhooshMotion.undoRise).combined(with: .opacity))
            }
            if !state.isSent { ComposerPhoneBar(state: state, speech: PhoneSpeech.shared).disabled(cm.isCounting) }
        }
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: cm.isCounting)
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: state.isSent)
    }

    // MARK: the send window

    /// The core is the truth about the entry: while the window is open this asks whether it closed, then the model says "Sent".
    private func watchWindow() async {
        while !Task.isCancelled, cm.outboxId != nil {
            try? await Task.sleep(for: .milliseconds(Self.reconcileMs))
            cm.reconcile(m)
        }
    }

    /// "Sent" stays for the Mac composer's beat (READER_CROSSFADE_MS x 10), then the screen goes back to where it came from.
    private func closeAfterSent() async {
        guard state.isSent else { return }
        try? await Task.sleep(for: .milliseconds(WhooshConstants.READER_CROSSFADE_MS * 10))
        if state.isSent { PhoneSpeech.shared.reset(); state.close(m) }
    }
}

/// Cancel, the title, Send: Cancel and Send are glass over the paper, the title is the Mac's word for the kind of message.
struct ComposerPhoneTop: View {
    let state: ComposerPhoneState
    var onCancel: () -> Void
    var onSend: () -> Void

    var body: some View {
        let cm = state.model
        let can = cm.canSend
        HStack(spacing: WhooshSpacing.s8) {
            Button(action: onCancel) {
                Text("Cancel").phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
                    .frame(width: PhoneMetrics.selectWidth, height: PhoneMetrics.roundButton).whooshGlass(in: Capsule())
            }
            .buttonStyle(PressStyle()).disabled(cm.isCounting).accessibilityIdentifier(ComposerPhoneIds.cancel)
            Spacer(minLength: 0)
            Text(ComposerWords.title(cm.kind)).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label).lineLimit(1)
                .accessibilityAddTraits(.isHeader)
            Spacer(minLength: 0)
            Button(action: onSend) {
                Image(systemName: "arrow.up").font(.system(size: PhoneMetrics.roundButton * 0.42, weight: .bold))
                    .foregroundStyle(can ? AnyShapeStyle(Color.white) : AnyShapeStyle(WhooshPalette.tertiaryLabel))
                    .frame(width: PhoneMetrics.roundButton, height: PhoneMetrics.roundButton)
                    .background { if can { Circle().fill(WhooshPalette.accent) } }
                    .whooshGlass(in: Circle())
            }
            .buttonStyle(PressStyle()).disabled(cm.isCounting)
            .keyboardShortcut(.return, modifiers: .command)
            .accessibilityLabel(Words.text("Send")).accessibilityHint(Words.text("Command Return"))
            .accessibilityIdentifier(ComposerPhoneIds.send)
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s8)
    }
}
