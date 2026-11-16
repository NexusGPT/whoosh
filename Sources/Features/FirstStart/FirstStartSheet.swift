import SwiftUI

/// Steps 6 and 9: the sheet itself is the step. Step 6 mounts the inline add-account card (AddAccountView, `style = .inline`);
/// step 9 mounts the import panel (ImportPanelView). Both sit in the quiet glass at the add-account sheet's width, with Back, the
/// count, Skip (and Finish at 7) under them (1a steps 6 and 7; 2b `under`). Nothing here re-draws what those two views draw.
struct FirstStartSheet: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    let step: Int
    @State private var adding = AddAccountModel.shared
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let rect = geo.card(step: step)
        let shape = RoundedRectangle(cornerRadius: 16, style: .continuous)
        VStack(spacing: 14) {
            Group {
                if step == FirstStartModel.addStep { AddAccountView() } else { ImportPanelView() }
            }
            .padding(.horizontal, 24).padding(.top, 22).padding(.bottom, 18)
            .frame(width: rect.width)
            .background(shape.fill(FirstStartLook.cardTint.color(scheme)))
            .whooshGlass(in: shape)
            .shadow(color: .black.opacity(scheme == .dark ? 0.45 : 0.14), radius: 24, y: 12)
            .accessibilityIdentifier("\(FirstStartFeature.X).sheet.\(step)")
            under(width: rect.width)
        }
        .offset(x: rect.minX, y: rect.minY)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onChange(of: adding.added.count) { _, _ in model.accountsMayHaveChanged() }
        .onReceive(NotificationCenter.default.publisher(for: FixtureEvents.name("add-account"))) { _ in model.accountsMayHaveChanged() }
        .onChange(of: ImportModel.shared.skipped) { _, now in if now && model.step == FirstStartModel.importStep { _ = model.finish() } }
    }

    private func under(width: CGFloat) -> some View {
        HStack(spacing: 8) {
            FirstStartButton(id: "back", title: FirstStartWords.back, key: "⌘[") { _ = model.back() }
            Spacer(minLength: 0)
            Text(FirstStartWords.countOf(step)).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel)
                .accessibilityIdentifier("\(FirstStartFeature.X).count")
            Spacer(minLength: 0)
            FirstStartButton(id: "skip", title: FirstStartWords.skip, key: "esc") { _ = model.skip() }
            if step == FirstStartModel.importStep {
                FirstStartButton(id: "continue", title: FirstStartWords.finish, key: "↩", primary: true) { _ = model.next() }
            } else if model.accountCount > 0 {
                FirstStartButton(id: "continue", title: FirstStartWords.continueWord, key: "↩", primary: true) { _ = model.next() }
            }
        }
        .frame(width: width)
    }
}

/// Skip at step 6 (owner card 2026-11-09 11:43): the veil lifts on an empty inbox with an Add account button; steps 7 and 8 are skipped.
/// The card sits where the list's rows would be (2b `skip`); the rest of the window is the real one, and it answers clicks.
struct FirstStartEmptyInbox: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let list = geo.pane("list")
        VStack(spacing: 6) {
            Text(FirstStartWords.noAccount).font(.system(size: 14, weight: .semibold)).foregroundStyle(WhooshPalette.label)
            Text(FirstStartWords.fillInbox).font(.system(size: 13)).foregroundStyle(WhooshPalette.secondaryLabel)
            FirstStartButton(id: "add-account", title: FirstStartWords.addAccount, key: "↩", primary: true) { _ = model.addAccountFromEmptyInbox() }
                .padding(.top, 8)
        }
        .padding(20)
        .frame(width: list.width - 2)
        .background(WhooshPalette.background)
        .offset(x: list.minX + 1, y: list.height * 0.36)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .transition(.opacity)
        .accessibilityIdentifier("\(FirstStartFeature.X).empty-inbox")
    }
}

/// Step 9: one aurora ring crosses the window as the veil lifts (2b `onStep`): from 0.12 to full size, opacity 0.95 to 0, the send wave's
/// colours. It follows the LAND_AURORA_MS precedent: decorative, it gates nothing and takes no click; Reduced Motion draws none.
struct FirstStartRing: View {
    let model: FirstStartModel
    let size: CGSize
    @State private var run = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let d = (size.width * size.width + size.height * size.height).squareRoot() * 1.1
        let id = model.ringId
        Circle()
            .fill(RadialGradient(stops: stops, center: .center, startRadius: 0, endRadius: d / 2))
            .frame(width: d, height: d)
            .scaleEffect(run ? 1 : 0.12)
            .opacity(run ? 0 : 0.95)
            .position(x: size.width / 2, y: size.height / 2)
            .allowsHitTesting(false)
            .onAppear {
                if reduceMotion || model.frozen { model.ringDone(id); return }
                withAnimation(.timingCurve(0.2, 0.8, 0.2, 1, duration: FirstStartLook.ringS)) { run = true }
                Task { try? await Task.sleep(for: .milliseconds(WhooshConstants.LAND_AURORA_MS + 40)); model.ringDone(id) }
            }
            .accessibilityHidden(true)
    }

    /// 2b `.ringx`: transparent to 78 %, blue 86, purple 90, pink 94, clear at 100
    private var stops: [Gradient.Stop] {
        let c = FirstStartLook.aurora
        return [.init(color: .clear, location: 0), .init(color: .clear, location: 0.78), .init(color: c[0].color(scheme), location: 0.86),
                .init(color: c[1].color(scheme), location: 0.90), .init(color: c[2].color(scheme), location: 0.94), .init(color: .clear, location: 1)]
    }
}
