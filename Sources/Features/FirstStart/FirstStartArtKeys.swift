import SwiftUI

/// Step 3 (2b, with 2a's real-row try-it): glass sample cards and big glass key caps on the stage. J and K move the focus, E bursts the
/// focused card into 34 dot streaks flying right while the rest close up, H does the same upward after "Tomorrow 08:00", / rings the
/// field; then the cards return (8 s, last frame = first). The first key the user presses stops the loop: three sample rows answer
/// J, K, E and H on the frame (focus 80 ms, a row leaves in 120 ms, the rest close up in 140 ms). No other key moves anything.
struct FirstStartKeysArt: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    let t: Double
    @Environment(\.colorScheme) private var scheme
    @State private var flash = ""
    @State private var capsule = ""
    @State private var burst: (start: Date, box: CGRect, dir: CGPoint)?

    private static let caps: [(key: String, times: [Double])] = [("E", [2.7]), ("H", [3.6]), ("C", [5.2]), ("/", [4.6]), ("J", [0.6, 2.0]), ("K", [1.3])]
    private var started: Bool { model.keys.started }

    var body: some View {
        let w = geo.listW
        let top = FirstStartGeometry.stageTop
        ZStack(alignment: .topLeading) {
            if started { rows(w: w, top: top) } else { loopCards(w: w, top: top) }
            caps(w: w, top: top)
            capsules(w: w, top: top)
            FirstStartSampleTag().offset(x: w - 16 - 84, y: top + 4)
        }
        .allowsHitTesting(false)
        .onChange(of: model.keys.tick) { _, _ in answerKey(w: w, top: top) }
    }

    // MARK: the loop's cards (2b step 3)
    private func loopCards(w: CGFloat, top: CGFloat) -> some View {
        let ids = ["m1", "m2", "m3", "m4"]
        let wins: [[(Double, Double)]] = [[(0, 0.6), (1.3, 2.0), (6.6, 8)], [(0.6, 1.3), (2.0, 2.7)], [(2.7, 3.95)], [(3.95, 6.6)]]
        let opacity: [Double] = [1,
            FirstStartLook.track(t, [(0, 1), (2.7, 1), (2.82, 0), (6.6, 0), (6.78, 1)]),
            FirstStartLook.track(t, [(0, 1), (2.7, 1), (2.88, 1), (3.95, 1), (4.07, 0), (6.6, 0), (6.78, 1)]),
            1]
        let shift: [Double] = [0, 0,
            FirstStartLook.track(t, [(0, 0), (2.7, 0), (2.88, -64), (3.95, -64), (4.07, -64), (6.6, 0)]),
            FirstStartLook.track(t, [(0, 0), (2.7, 0), (2.88, -64), (3.95, -64), (4.13, -128), (6.6, -128), (6.78, 0)])]
        let box = CGRect(x: 24, y: top + 24 + 64, width: w - 48, height: 56)
        return ZStack(alignment: .topLeading) {
            ForEach(0..<4, id: \.self) { i in
                if let line = FirstStartSample.line(ids[i]) {
                    FirstStartSampleLine(line: line, selected: FirstStartLook.wins(t, wins[i]))
                        .frame(width: w - 48, height: 56)
                        .opacity(opacity[i])
                        .offset(x: 24, y: top + 24 + CGFloat(i) * 64 + CGFloat(shift[i]))
                }
            }
            chip(w: w, top: top)
            Canvas { ctx, _ in
                let c = FirstStartLook.aurora.map { $0.color(scheme) }
                FirstStartParticles.shatter(ctx, t: t, colours: c, n: 34, seed: 11, box: box, at: 2.7, dir: CGPoint(x: 180, y: 0))
                FirstStartParticles.shatter(ctx, t: t, colours: c, n: 34, seed: 23, box: box, at: 3.95, dir: CGPoint(x: 90, y: -120))
            }
        }
    }

    private func chip(w: CGFloat, top: CGFloat) -> some View {
        tomorrowChip.opacity(FirstStartLook.win(t, 3.6, 3.95, 0.12))
            .offset(x: w - 24 - 12 - 120, y: top + 24 + 64 + 10)
    }

    private var tomorrowChip: some View {
        HStack(spacing: 5) {
            Image(systemName: "clock").font(.system(size: 11)).foregroundStyle(WhooshPalette.secondaryLabel)
            Text(FirstStartWords.tomorrowChip).font(.system(size: 11, weight: .medium)).foregroundStyle(WhooshPalette.label)
        }
        .padding(.horizontal, 8).frame(height: 20)
        .background(WhooshPalette.background, in: Capsule())
    }

    // MARK: the try-it's rows
    private func rows(w: CGFloat, top: CGFloat) -> some View {
        let k = model.keys
        return ZStack(alignment: .topLeading) {
            ForEach(Array(k.rows.enumerated()), id: \.element) { i, id in
                if let line = FirstStartSample.line(id) {
                    FirstStartSampleLine(line: line, selected: i == k.focus ? 1 : 0)
                        .frame(width: w - 48, height: 56)
                        .offset(x: 24, y: top + 24 + CGFloat(i) * 64)
                        .transition(.opacity.combined(with: .offset(x: -18)).animation(.easeOut(duration: FirstStartLook.leaveS)))
                }
            }
            TimelineView(.animation(paused: burst == nil)) { ctx in
                Canvas { gc, _ in
                    guard let b = burst else { return }
                    let c = FirstStartLook.aurora.map { $0.color(scheme) }
                    FirstStartParticles.shatter(gc, t: ctx.date.timeIntervalSince(b.start), colours: c, n: 34, seed: 11, box: b.box, at: 0, dir: b.dir)
                }
            }
        }
        .animation(model.frozen ? nil : .easeOut(duration: FirstStartLook.closeUpS), value: k.rows)
        .animation(model.frozen ? nil : .easeOut(duration: FirstStartLook.keyPressS), value: k.focus)
    }

    private func answerKey(w: CGFloat, top: CGFloat) {
        let k = model.keys
        flash = k.lastKey
        Task { try? await Task.sleep(for: .milliseconds(WhooshConstants.FOCUS_SLIDE_MS + 40)); flash = "" }
        guard k.last == "done" || k.last == "remind" else { return }
        capsule = k.last == "done" ? "done" : "remind"
        let box = CGRect(x: 24, y: top + 24 + CGFloat(k.focus) * 64, width: w - 48, height: 56)
        if !model.frozen { burst = (Date(), box, k.last == "done" ? CGPoint(x: 180, y: 0) : CGPoint(x: 90, y: -120)) }
        Task { try? await Task.sleep(for: .milliseconds(1100)); capsule = ""; burst = nil }
    }

    // MARK: the caps and the capsules
    private func caps(w: CGFloat, top: CGFloat) -> some View {
        let big = w >= 400
        let cs: CGFloat = big ? 46 : 40, gap: CGFloat = big ? 10 : 8
        let total = 6 * cs + 5 * gap
        let labels = [FirstStartWords.capDone, FirstStartWords.capRemind, FirstStartWords.capCompose, FirstStartWords.capSearch, FirstStartWords.capDown, FirstStartWords.capUp]
        return HStack(alignment: .top, spacing: gap) {
            ForEach(0..<6, id: \.self) { i in cap(Self.caps[i].key, labels[i], Self.caps[i].times, cs: cs, big: big) }
        }
        .frame(width: total)
        .offset(x: (w - total) / 2, y: top + 24 + 4 * 64 + 16)
    }

    private func cap(_ key: String, _ label: String, _ times: [Double], cs: CGFloat, big: Bool) -> some View {
        let pressed = started ? (flash == key ? 1.0 : 0.0) : times.map { pressAt($0) }.max() ?? 0
        let lit = started ? (flash == key ? 1.0 : 0.0) : times.map { lightAt($0) }.max() ?? 0
        let shape = RoundedRectangle(cornerRadius: 11, style: .continuous)
        return VStack(spacing: 6) {
            ZStack {
                Text(key).font(.system(size: 16, weight: .semibold)).foregroundStyle(WhooshPalette.label)
                shape.fill(WhooshPalette.accent).opacity(lit).overlay { Text(key).font(.system(size: 16, weight: .semibold)).foregroundStyle(WhooshPalette.onAccent).opacity(lit) }
            }
            .frame(width: cs, height: cs - 2)
            .firstStartSample(radius: 11)
            .scaleEffect(1 - 0.1 * pressed)
            Text(label).font(.system(size: big ? 11 : 10)).foregroundStyle(WhooshPalette.label).opacity(0.8).lineLimit(1).fixedSize()
        }
        .frame(width: cs)
    }

    /// 2b `PRESS`: 1 to 0.9 in 60 ms, back to 1 by 200 ms after the key's moment
    private func pressAt(_ at: Double) -> Double { FirstStartLook.run(t, at, at + 0.06) * (1 - FirstStartLook.run(t, at + 0.06, at + 0.2)) }
    /// 2b `LIGHT`: the cap lights in 50 ms, holds to 380 ms, is out by 560 ms
    private func lightAt(_ at: Double) -> Double { FirstStartLook.run(t, at, at + 0.05) * (1 - FirstStartLook.run(t, at + 0.38, at + 0.56)) }

    private func capsules(w: CGFloat, top: CGFloat) -> some View {
        let cs: CGFloat = w >= 400 ? 46 : 40
        let y = top + 24 + 4 * 64 + cs + 46
        let doneOn = started ? (capsule == "done" ? 1.0 : 0.0) : FirstStartLook.win(t, 2.75, 3.85, 0.18)
        let remindOn = started ? (capsule == "remind" ? 1.0 : 0.0) : FirstStartLook.win(t, 3.95, 4.95, 0.18)
        return ZStack {
            capsuleView(icon: "checkmark", words: AnyView(HStack(spacing: 6) { Text(FirstStartWords.markedDone); FirstStartKbd(text: "Z"); Text(FirstStartWords.undo) }), on: doneOn)
            capsuleView(icon: "clock", words: AnyView(Text(FirstStartWords.reminderTomorrow)), on: remindOn)
        }
        .frame(width: 220, height: 30)
        .offset(x: (w - 220) / 2, y: y)
    }

    private func capsuleView(icon: String, words: AnyView, on: Double) -> some View {
        HStack(spacing: 8) {
            Image(systemName: icon).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel)
            words
        }
        .font(.system(size: 12.5)).foregroundStyle(WhooshPalette.label)
        .padding(.leading, 12).padding(.trailing, 6)
        .frame(width: 220, height: 30)
        .firstStartSample(radius: 15)
        .opacity(on).offset(y: 8 * (1 - on))
    }
}
