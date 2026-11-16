import SwiftUI

/// The bar above the keyboard: Attach, and the Dictate pill (the inbox's search pill, same glass, same sizes). The pill is the microphone:
/// idle it invites, downloading it counts up the speech model, listening it shows the words as they come, then the text lands in the body.
struct ComposerPhoneBar: View {
    let state: ComposerPhoneState
    @Bindable var speech: PhoneSpeech
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        _ = speech.revision                                  // the controller's changes redraw the pill
        return HStack(spacing: PhoneMetrics.barGap) {
            Button { state.attaching.toggle() } label: { glyph("paperclip") }
                .buttonStyle(PressStyle()).accessibilityLabel("Attach a file").accessibilityIdentifier(ComposerPhoneIds.attach)
            pill
        }
        .padding(.horizontal, PhoneMetrics.gutter)
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: speech.mic)
    }

    private var pill: some View {
        Button { speech.tap() } label: {
            HStack(spacing: WhooshSpacing.s8) {
                status
                Spacer(minLength: 0)
                micGlyph.frame(width: PhoneMetrics.roundButton, height: PhoneMetrics.roundButton)
            }
            .padding(.leading, PhoneMetrics.gutter).padding(.trailing, WhooshSpacing.s4)
            .frame(maxWidth: .infinity).frame(height: PhoneMetrics.barButton)
            .whooshGlass(in: Capsule()).contentShape(Capsule())
        }
        .buttonStyle(PressStyle())
        .accessibilityLabel(label)
        .accessibilityValue(speech.mic == .downloading ? "\(speech.percent)%" : "")
        .accessibilityIdentifier(ComposerPhoneIds.dictate)
    }

    @ViewBuilder private var status: some View {
        switch speech.mic {
        case .idle, .text:
            if speech.line.isEmpty { Text("Dictate").phoneText(PhoneType.body).foregroundStyle(WhooshPalette.secondaryLabel) }
            else { Text(speech.line).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(2) }
        case .downloading:
            Text("Speech model").phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label)
            if case .downloading(let p) = speech.model, p != nil {
                Text("\(speech.percent)%").phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel).contentTransition(.numericText())
            }
        case .listening:
            Text(speech.liveWords.isEmpty ? Words.text("Listening") : speech.liveWords).phoneText(PhoneType.body)
                .foregroundStyle(WhooshPalette.label).lineLimit(1).truncationMode(.head)
        case .cleaning:
            Text("Writing").phoneText(PhoneType.body).foregroundStyle(WhooshPalette.secondaryLabel)
        }
    }

    @ViewBuilder private var micGlyph: some View {
        switch speech.mic {
        case .downloading:
            ZStack {
                Circle().stroke(WhooshPalette.separator, lineWidth: PhoneMetrics.tileStroke)
                Circle().trim(from: 0, to: speech.progress).stroke(WhooshPalette.selection, style: StrokeStyle(lineWidth: PhoneMetrics.tileStroke, lineCap: .round))
                    .rotationEffect(.degrees(-90))
            }
            .frame(width: WhooshSpacing.s24, height: WhooshSpacing.s24)
        case .listening:
            Image(systemName: "waveform").font(.system(size: PhoneType.headline.size, weight: .semibold)).foregroundStyle(WhooshPalette.selection)
                .symbolEffect(.variableColor.iterative, isActive: !reduceMotion)
        case .cleaning:
            ProgressView()
        case .idle, .text:
            Image(systemName: "mic").font(.system(size: PhoneType.headline.size, weight: .semibold)).foregroundStyle(WhooshPalette.label)
        }
    }

    private var label: String {
        switch speech.mic {
        case .idle, .text: Words.text("Dictate")
        case .downloading: Words.text("Downloading the speech model")
        case .listening: Words.text("Stop dictation")
        case .cleaning: Words.text("Writing")
        }
    }

    private func glyph(_ symbol: String) -> some View {
        Image(systemName: symbol).font(.system(size: PhoneType.title3.size + WhooshSpacing.s2, weight: .semibold)).foregroundStyle(WhooshPalette.label)
            .frame(width: PhoneMetrics.barButton, height: PhoneMetrics.barButton).whooshGlass(in: Circle())
    }
}

/// The capsule above the bar once Send is tapped: "Sending" with its window running out as a ring, and Undo; then "Sent" for a beat.
/// The window is the core's (`UndoCountdown` over the outbox entry, UNDO_SEND_DEFAULT_S unless the entry says otherwise).
struct ComposerPhoneUndo: View {
    let state: ComposerPhoneState
    var onUndo: () -> Void

    var body: some View {
        Group {
            if state.isSent {
                HStack(spacing: WhooshSpacing.s8) {
                    Image(systemName: "checkmark").foregroundStyle(WhooshPalette.selection)
                    Text("Sent").foregroundStyle(WhooshPalette.label)
                    Spacer(minLength: 0)
                }
            } else if case .counting(_, let c) = state.model.phase {
                HStack(spacing: WhooshSpacing.s8) {
                    TimelineView(.animation(minimumInterval: 0.1)) { ctx in
                        Circle().trim(from: 0, to: c.fraction(at: ctx.date))
                            .stroke(WhooshPalette.selection, style: StrokeStyle(lineWidth: PhoneMetrics.tileStroke, lineCap: .round))
                            .rotationEffect(.degrees(-90))
                    }
                    .frame(width: WhooshSpacing.s20, height: WhooshSpacing.s20)
                    Text("Sending").foregroundStyle(WhooshPalette.label)
                    Spacer(minLength: 0)
                    Button(action: onUndo) {
                        Text("Undo").phoneText(PhoneType.subhead, emphasized: true).foregroundStyle(WhooshPalette.selection)
                            .padding(.horizontal, WhooshSpacing.s14).frame(height: PhoneMetrics.roundButton).contentShape(Capsule())
                    }
                    .buttonStyle(PressStyle()).accessibilityIdentifier(ComposerPhoneIds.undoCancel)
                }
            }
        }
        .phoneText(PhoneType.subhead)
        .padding(.leading, WhooshSpacing.s20).padding(.trailing, WhooshSpacing.s2)
        .frame(width: PhoneMetrics.undoWidth, height: PhoneMetrics.roundButton)
        .whooshGlass(in: Capsule())
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(ComposerPhoneIds.undo)
    }
}
