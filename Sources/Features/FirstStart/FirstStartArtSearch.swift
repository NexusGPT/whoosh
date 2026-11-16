import SwiftUI

/// Step 2 (2b): the stage's search field types "boat"; 26 loose dots gather into one glass card, the Sailing club line, found by meaning;
/// later the card dissolves and the dots scatter back. 8 s, last frame = first. Typing a word into the field stops the loop and answers it
/// from the sample index (the try-it, J9.2): a line when a word means one, a short line when none does.
struct FirstStartSearchArt: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    let t: Double
    @Environment(\.colorScheme) private var scheme
    @FocusState private var focused: Bool

    private var typed: Bool { !model.tryText.isEmpty }

    var body: some View {
        let w = geo.listW
        let top = FirstStartGeometry.stageTop
        let cardY = top + 120
        ZStack(alignment: .topLeading) {
            if !typed {
                Canvas { ctx, _ in
                    let colours = FirstStartLook.aurora.map { $0.color(scheme) }
                    FirstStartParticles.converge(ctx, t: t, colours: colours, n: 26, seed: 7,
                                                 area: CGRect(x: 20, y: top + 40, width: w - 40, height: min(440, geo.size.height - top - 70)),
                                                 target: CGPoint(x: w / 2, y: cardY + 40), tIn: 1.75, tOut: 6.2)
                }.allowsHitTesting(false)
                if let hit = FirstStartSample.line("m13") { loopCard(hit, w: w, y: cardY) }
            } else {
                answer(w: w, y: cardY)
            }
            field(w: w)
            FirstStartSampleTag().offset(x: w - 16 - 84, y: top + 12).allowsHitTesting(false)
        }
        .onAppear { focused = true }
    }

    // MARK: the card of the loop
    private func loopCard(_ line: FirstStartLine, w: CGFloat, y: CGFloat) -> some View {
        let show = FirstStartLook.run(t, 1.95, 2.15) * (1 - FirstStartLook.run(t, 6.2, 6.32))
        let scale = t < 6.2 ? 0.96 + 0.04 * FirstStartLook.run(t, 1.95, 2.15) : 1 + 0.02 * FirstStartLook.run(t, 6.2, 6.32)
        return FirstStartSampleLine(line: line, third: AnyView(why(FirstStartWords.foundByMeaning("boat", line.sender))))
            .frame(width: w - 48, height: 96)
            .scaleEffect(scale).opacity(show)
            .offset(x: 24, y: y)
            .allowsHitTesting(false)
    }

    private func why(_ text: String) -> some View {
        HStack(spacing: 6) {
            Image(systemName: "sparkles").font(.system(size: 11)).foregroundStyle(WhooshPalette.accent)
            Text(text).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
        }
        .padding(.top, 8).overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: 0.5) }
    }

    // MARK: the try-it's answer
    @ViewBuilder private func answer(w: CGFloat, y: CGFloat) -> some View {
        if let hit = FirstStartSample.match(model.tryText) {
            FirstStartSampleLine(line: hit, third: AnyView(why(FirstStartWords.foundByMeaning(model.tryText, hit.sender))))
                .frame(width: w - 48, height: 96).offset(x: 24, y: y).transition(.opacity)
                .accessibilityIdentifier("\(FirstStartFeature.X).search.result")
        } else if model.tryText.count >= 2 {
            Text(FirstStartWords.noSample(model.tryText)).font(.system(size: 12.5)).foregroundStyle(WhooshPalette.secondaryLabel)
                .padding(.horizontal, 14).frame(width: w - 48, height: 44, alignment: .leading).firstStartSample(radius: 12)
                .offset(x: 24, y: y).transition(.opacity)
        }
    }

    // MARK: the field (the board's `.qf`): the real search field drawn on the stage, with the accent ring
    private func field(w: CGFloat) -> some View {
        let shape = RoundedRectangle(cornerRadius: 8, style: .continuous)
        return HStack(spacing: 8) {
            Image(systemName: "magnifyingglass").font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel)
            ZStack(alignment: .leading) {
                if !typed { loopText }
                TextField("", text: Binding(get: { model.tryText }, set: { model.tryText = $0 }))
                    .textFieldStyle(.plain).font(.system(size: 13)).focused($focused)
                    // Return in the field is the step's Continue (owner, 16 Nov 2026 on main203: "shortcut don't work on quickstart, I cannot press enter"):
                    // the field holds the keyboard from the step's first frame, and a focused text keeps Return for itself, so the flow never saw the key.
                    .onSubmit { model.next() }
                    .opacity(typed ? 1 : 0.015)
                    .accessibilityLabel(Text(FirstStartWords.searchPlaceholder))
                    .accessibilityIdentifier("\(FirstStartFeature.X).search.field")
            }
            Spacer(minLength: 0)
            FirstStartKbd(text: "⌘K")
        }
        .padding(.leading, 10).padding(.trailing, 6)
        .frame(width: w - 32, height: 30)
        .background(shape.fill(WhooshPalette.background))
        .overlay(shape.strokeBorder(WhooshPalette.accent.opacity(0.75), lineWidth: 1))
        .overlay(shape.stroke(WhooshPalette.accent.opacity(0.24), lineWidth: 3.5).padding(-1.75))
        .offset(x: 16, y: 82)
    }

    /// the loop's typing: the placeholder until 0.9 s, then b, o, a, t at 1.0 s and 160 ms apart, held to 6.2 s, cleared by 6.5 s
    private var loopText: some View {
        let word = Array("boat")
        let shown = word.indices.filter { t >= 1.0 + Double($0) * 0.16 }.count
        let fadeOut = FirstStartLook.run(t, 6.2, 6.32)
        let placeholder = (t < 0.9 ? 1 : 0) + FirstStartLook.run(t, 6.35, 6.5)
        return ZStack(alignment: .leading) {
            Text(FirstStartWords.searchPlaceholder).font(.system(size: 13)).foregroundStyle(WhooshPalette.secondaryLabel).opacity(min(placeholder, 1))
            HStack(spacing: 0) {
                Text(String(word.prefix(shown))).font(.system(size: 13)).foregroundStyle(WhooshPalette.label)
                Rectangle().fill(WhooshPalette.accent).frame(width: 1.5, height: 16).opacity(t > 0.9 && t < 6.2 && Int(t / 0.3) % 2 == 0 ? 1 : 0)
            }.opacity(1 - fadeOut)
        }.allowsHitTesting(false)
    }
}
