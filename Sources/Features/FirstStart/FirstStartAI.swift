import SwiftUI

/// Step 7, "AI" (design Q1 of ai-setup-r1, "One card, three lines"): eyebrow, title, one sentence, three lines (Sorting, Writing and asking, Search by
/// meaning) each with its state and its one action, the coach cards' dots and Back / Skip / Continue. Continue is always on and Skip leaves the
/// warn marks on (they read AISetupStatus, never this step). The stage in the list pane says "Sorted, written, found." with three rings that fill
/// as each need is met. Every action is an existing door (AISetupStatus); only opacity and transform move.
struct FirstStartAICard: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let rect = geo.card(step: FirstStartModel.aiStep)
        let shape = RoundedRectangle(cornerRadius: 14, style: .continuous)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 6) { Image(systemName: "sparkles").font(.system(size: 11, weight: .semibold)); Text(AISetupWords.stepKicker) }
                .font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
            Text(AISetupWords.stepTitle).font(.system(size: 20, weight: .semibold)).tracking(-0.35).lineSpacing(2)
                .foregroundStyle(WhooshPalette.label).padding(.top, 6).padding(.bottom, 6)
            Text(AISetupWords.stepBody).font(.system(size: 13)).lineSpacing(3).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
            FirstStartAILines().padding(.top, 8)
            Spacer(minLength: 0)
            HStack {
                FirstStartDots(current: FirstStartModel.aiStep, still: model.frozen)
                Spacer(minLength: 0)
                Text(FirstStartWords.stepOf(FirstStartModel.aiStep)).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.tertiaryLabel)
            }.padding(.top, 12).padding(.bottom, 12)
            HStack(spacing: 8) {
                FirstStartButton(id: "back", title: FirstStartWords.back, key: "⌘[") { _ = model.back() }
                Spacer(minLength: 0)
                FirstStartButton(id: "skip", title: FirstStartWords.skip, key: "esc") { _ = model.skip() }
                FirstStartButton(id: "continue", title: FirstStartWords.continueWord, key: "↩", primary: true) { _ = model.next() }
            }
        }
        .padding(.horizontal, 20).padding(.top, 18).padding(.bottom, 16)
        .frame(width: rect.width, height: rect.height)
        .background(shape.fill(FirstStartLook.cardTint.color(scheme)))
        .background(shape.fill(WhooshPalette.background))   // opaque under the 0.74 tint: this card is shown after the first account exists, over a list and a reader that hold its mail (main105: the mail read through the card)
        .whooshGlass(in: shape)
        .whooshAx("\(FirstStartFeature.X).card")   // on the card itself: its frame is the card's (it was the whole window's, so a check of the card read the mail around it; main170, welcome-ai-step)
        .shadow(color: .black.opacity(scheme == .dark ? 0.45 : 0.14), radius: 24, y: 12)
        .firstStartEntrance(from: 0.98, rise: 12, duration: FirstStartLook.morphS, delay: 0.04, still: model.frozen)
        .offset(x: rect.minX, y: rect.minY)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

/// The three lines of the board's `.qrow`: a 26 pt icon tile, a title with its small line, then the action on the right.
struct FirstStartAILines: View {
    @State private var status = AISetupStatus.shared
    @State private var step = AISetupStep.shared
    @State private var keyText = ""
    @State private var jevText = ""
    private let P = "\(FirstStartFeature.X).ai"

    var body: some View {
        VStack(spacing: 0) {
            sorting
            row("meaning", icon: "magnifyingglass", title: AISetupWords.searchByMeaning, small: AISetupWords.meaningLine, need: .meaning) { x in
                if x.state == .ready { ok(AISetupWords.ready) }
                else if x.state != .working { AISetupButton(title: x.state == .failed ? AISetupWords.retry : AISetupWords.doorDownload, primary: true) { AISetupDoors.fix(.meaning) }.whooshAx("\(P).meaning.download") }
            }
            .overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: 0.5) }
            writing
        }
        .task { status.refresh() }
    }

    // MARK: Sorting
    private var sorting: some View {
        row("sorting", icon: "brain", title: AISetupWords.sorting, small: sortingSmall, need: .decision) { x in
            switch (x.state, step.jev) {
            case (.ready, _): ok(AISetupWords.ready)
            case (.working, _): AISetupButton(title: AISetupWords.cancel, amber: false) { AISetupDoors.cancel(.decision) }.whooshAx("\(P).sorting.cancel")
            case (_, .key):
                HStack(spacing: 6) {
                    AISetupField(placeholder: AISetupWords.pasteJevKey, text: $jevText) { step.submitJevKey(jevText); jevText = "" }.whooshAx("\(P).sorting.jev-key")
                }
            case (_, .consent): AISetupButton(title: AISetupWords.allow, primary: true) { step.allowJev() }.whooshAx("\(P).sorting.jev-allow")
            default:
                HStack(spacing: 8) {
                    AISetupButton(title: AISetupWords.useJev) { step.openJev() }.whooshAx("\(P).sorting.use-jev")
                    AISetupButton(title: x.state == .failed ? AISetupWords.retry : AISetupWords.doorDownload, primary: true) { step.download() }.whooshAx("\(P).sorting.download")
                }
            }
        }
    }

    private var sortingSmall: String {
        let x = status.status(.decision)
        if step.jev == .consent { return AISetupWords.jevSent }
        if x.state == .ready { return x.words }
        if x.state == .working || x.state == .failed { return x.words }
        return AISetupWords.sortingLine
    }

    // MARK: Writing and asking
    private var writing: some View {
        row("writing", icon: "pencil.line", title: AISetupWords.writingAsking, small: writingSmall, need: .writing) { x in
            if x.state == .ready { ok(AISetupWords.connected, green: true) }
            else { AISetupField(placeholder: AISetupWords.pasteKey, text: $keyText) { step.pasteWritingKey(keyText); keyText = "" }.whooshAx("\(P).writing.key") }
        }
        .overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: 0.5) }
    }

    private var writingSmall: String {
        let x = status.status(.writing)
        if x.state == .ready { return x.detail.isEmpty ? x.words : x.detail }
        if x.state == .working || x.reason == "provider-refused" { return x.words }
        return AISetupWords.writingLine
    }

    // MARK: one line
    private func row<Action: View>(_ id: String, icon: String, title: String, small: String, need: AINeed, @ViewBuilder action: (AINeedStatus) -> Action) -> some View {
        let x = status.status(need)
        return HStack(spacing: 10) {
            Image(systemName: icon).font(.system(size: 13)).foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(width: 26, height: 26).background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            VStack(alignment: .leading, spacing: 0) {
                Text(title).font(.system(size: 13, weight: .semibold)).foregroundStyle(WhooshPalette.label)
                Text(AISetupWords.unbroken(small)).font(.system(size: 12)).foregroundStyle(x.state == .failed || x.reason == "provider-refused" ? WhooshPalette.warning : WhooshPalette.secondaryLabel)
                    .fixedSize(horizontal: false, vertical: true)
                if x.state == .working { AISetupBar(fraction: x.progress ?? 0).padding(.top, 8) }
            }
            Spacer(minLength: 8)
            action(x)
        }
        .padding(.vertical, 9)
        .whooshAx("\(P).\(id)")
    }

    private func ok(_ text: String, green: Bool = true) -> some View {
        Text(text).font(.system(size: 12, weight: .semibold)).foregroundStyle(Color(.sRGB, red: 0x30 / 255, green: 0xD1 / 255, blue: 0x58 / 255, opacity: 1))
    }
}

/// The board's `.fld`: 26 pt, 7 pt radius, the field fill, 12.5 pt; a secure field (a key is never shown); Return sends it.
struct AISetupField: View {
    let placeholder: String
    @Binding var text: String
    let submit: () -> Void
    var body: some View {
        SecureField(placeholder, text: $text).textFieldStyle(.plain).font(.system(size: 12.5)).onSubmit(submit)
            .padding(.horizontal, 8).frame(width: 170, height: 26)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
    }
}

/// The stage of the list pane (design Q1): "Sorted, written, found." and three lines with a ring each; a ring fills (opacity) as its need is met.
struct FirstStartAIArt: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    let t: Double
    @State private var status = AISetupStatus.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(AISetupWords.stageTitle).font(.system(size: 19, weight: .semibold)).tracking(-0.2).foregroundStyle(WhooshPalette.label).padding(.bottom, 0)
            line(AISetupWords.stageSorting, .decision)
            line(AISetupWords.stageWriting, .writing)
            line(AISetupWords.stageSearch, .meaning)
        }
        .opacity(0.92)
        .frame(width: geo.listW - 48, alignment: .leading)
        .offset(x: 24, y: FirstStartGeometry.stageTop + 22)
        .allowsHitTesting(false)
        .animation(WhooshMotion.animation(.easeOut(duration: Double(WhooshConstants.POPOVER_MS) / 1000), reduceMotion: reduceMotion || model.frozen), value: status.revision)
    }

    private func line(_ text: String, _ need: AINeed) -> some View {
        let met = status.status(need).usable
        return HStack(spacing: 10) {
            // the theme's text colours, as the line beside it: a white ring showed on the dark pane only (the lead's read of the light picture, main182). The empty
            // ring is the second text colour, solid: at 55 % of the first it read 3.3 to 1 on the light pane and 5.1 on the dark one (main185, par-firststart)
            ZStack {
                Circle().strokeBorder(WhooshPalette.secondaryLabel, lineWidth: 2)
                Circle().fill(WhooshPalette.label).scaleEffect(met ? 1 : 0.4).opacity(met ? 1 : 0)
            }.frame(width: 18, height: 18).whooshAx("\(FirstStartFeature.X).ai.ring.\(need.rawValue).mark")
            Text(text).font(.system(size: 13)).foregroundStyle(WhooshPalette.label)
        }
        .accessibilityElement(children: .combine)
        .whooshAx("\(FirstStartFeature.X).ai.ring.\(need.rawValue)")
    }
}
