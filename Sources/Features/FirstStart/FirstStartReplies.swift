import SwiftUI

/// Step 7, "Réponses" (the approved board ai-replies-r5, FS(...)): the coach card with three choices (when to write a draft, where to search, write
/// like whom) beside the list's stage, which shows an example draft. Continue writes the choices through the delta 20 commands (RepliesStepModel);
/// Skip leaves the defaults. The card keeps the coach cards' chrome; only opacity and transform move.
struct FirstStartRepliesCard: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let rect = geo.card(step: FirstStartModel.repliesStep)
        let shape = RoundedRectangle(cornerRadius: 14, style: .continuous)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 6) { Image(systemName: "sparkles").font(.system(size: 11, weight: .semibold)); Text(RepliesStepWords.kicker) }
                .font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
            Text(RepliesStepWords.title).font(.system(size: 20, weight: .semibold)).tracking(-0.35).lineSpacing(2)
                .foregroundStyle(WhooshPalette.label).padding(.top, 6).padding(.bottom, 6)
            Text(RepliesStepWords.body).font(.system(size: 13)).lineSpacing(3).foregroundStyle(WhooshPalette.secondaryLabel)
                .fixedSize(horizontal: false, vertical: true)
            FirstStartRepliesChoices().padding(.top, 10)
            Spacer(minLength: 0)
            HStack {
                FirstStartDots(current: FirstStartModel.repliesStep, still: model.frozen)
                Spacer(minLength: 0)
                Text(FirstStartWords.stepOf(FirstStartModel.repliesStep)).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.tertiaryLabel)
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

/// The three rows of the board: a title, its small line, and a pop-up at the right (the board's `.pop`: 24 pt tall, 7 pt radius).
struct FirstStartRepliesChoices: View {
    @State private var replies = RepliesStepModel.shared

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            line(RepliesStepWords.draftRow, nil, id: "draft", words: RepliesStepWords.draft(replies.draft),
                 options: RepliesDraftChoice.allCases.map { ($0.rawValue, RepliesStepWords.draft($0)) }) { id in
                if let c = RepliesDraftChoice(rawValue: id) { replies.draft = c }
            }
            line(RepliesStepWords.searchRow, RepliesStepWords.searchCaption, id: "search", words: RepliesStepWords.search(replies.search),
                 options: RepliesSearchChoice.allCases.map { ($0.rawValue, RepliesStepWords.search($0)) }) { id in
                if let c = RepliesSearchChoice(rawValue: id) { replies.search = c }
            }
            line(RepliesStepWords.voiceRow, RepliesStepWords.voiceCaption, id: "voice", words: RepliesStepWords.voice(replies.voice.rawValue),
                 options: RepliesVoiceChoice.allCases.map { (String($0.rawValue), RepliesStepWords.voice($0.rawValue)) }) { id in
                if let n = Int(id), let c = RepliesVoiceChoice(rawValue: n) { replies.voice = c }
            }
        }
    }

    private func line(_ title: String, _ caption: String?, id: String, words: String, options: [(String, String)], pick: @escaping (String) -> Void) -> some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.system(size: 13, weight: .semibold)).foregroundStyle(WhooshPalette.label)
                if let caption { Text(caption).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1) }
            }
            Spacer(minLength: 8)
            Menu {
                ForEach(options, id: \.0) { option in Button(option.1) { pick(option.0) } }
            } label: {
                HStack(spacing: 6) {
                    Text(words).font(.system(size: 13)).foregroundStyle(WhooshPalette.label).lineLimit(1)
                    Image(systemName: "chevron.up.chevron.down").font(.system(size: 8, weight: .bold)).foregroundStyle(WhooshPalette.onAccent)
                        .frame(width: 16, height: 16).background(WhooshPalette.accent, in: RoundedRectangle(cornerRadius: 4, style: .continuous))
                }
                .padding(.leading, 10).padding(.trailing, 4).frame(height: 24)
                .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
            }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .whooshAx("\(FirstStartFeature.X).replies.\(id)")
        }
        .padding(.vertical, 8)
        .overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: 0.5) }
    }
}

/// The stage of the list pane (the board's `stage`): "EXAMPLE" at the top right and one draft on dark glass, marked as a draft. Static content: the
/// card enters with the step (opacity and a short rise), nothing loops on it.
struct FirstStartRepliesArt: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    let t: Double

    var body: some View {
        let w = geo.listW
        ZStack(alignment: .topLeading) {
            // on the pane itself, so the theme's text colour (white showed on the dark pane only); the draft under it sits on its own dark glass and stays white
            Text(RepliesStepWords.example.uppercased()).font(.system(size: 11, weight: .semibold)).tracking(0.5).foregroundStyle(WhooshPalette.label.opacity(0.8))
                .whooshAx("\(FirstStartFeature.X).replies.example")
                .frame(width: w - 48, alignment: .trailing).offset(x: 24, y: FirstStartGeometry.stageTop + 22)
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 5) {
                    Image(systemName: "sparkles").font(.system(size: 11, weight: .semibold))
                    Text(RepliesStepWords.draftMark).font(.system(size: 12, weight: .semibold))
                }
                .foregroundStyle(.white).padding(.horizontal, 8).frame(height: 22).background(.white.opacity(0.16), in: Capsule())
                VStack(alignment: .leading, spacing: 8) {
                    Text(RepliesStepWords.exampleGreeting)
                    Text(RepliesStepWords.exampleText)
                }
                .font(.system(size: 14)).lineSpacing(3).foregroundStyle(.white).fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, 16).padding(.vertical, 14).frame(width: w - 48, alignment: .leading)
            .background(Color(.sRGB, red: 20 / 255, green: 22 / 255, blue: 30 / 255, opacity: 0.82), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.white.opacity(0.18), lineWidth: 0.5))
            .firstStartEntrance(from: 0.98, rise: 10, duration: FirstStartLook.morphS, delay: 0.12, still: model.frozen)
            .offset(x: 24, y: FirstStartGeometry.stageTop + 150)
        }
        .allowsHitTesting(false)
    }
}
