import SwiftUI

/// The words of each coach card: kicker, title, one sentence (2b, and 3a for step 1). One source for the card, the rows and the check.
@MainActor
enum FirstStartCardText {
    struct Copy { let kicker: String; let title: String; let body: String }
    static func of(_ step: Int) -> Copy {
        switch step {
        case 1: return Copy(kicker: FirstStartWords.welcomeKicker, title: FirstStartWords.appName, body: FirstStartWords.welcomeBody)
        case 2: return Copy(kicker: FirstStartWords.searchKicker, title: FirstStartWords.searchTitle, body: FirstStartWords.searchBody)
        case 3: return Copy(kicker: FirstStartWords.keysKicker, title: FirstStartWords.keysTitle, body: FirstStartWords.keysBody)
        case 4: return Copy(kicker: FirstStartWords.labelsKicker, title: FirstStartWords.labelsTitle, body: FirstStartWords.labelsBody)
        default: return Copy(kicker: FirstStartWords.privacyKicker, title: FirstStartWords.privacyTitle, body: FirstStartWords.privacyBody)
        }
    }
}

/// The coach card of steps 1 to 5: the lock's glass (blur, rim, soft shadow), one morph between anchors in MOTION_MAX_MS.
/// Its words cross-fade in 140 ms; no input waits on either (Enter, Esc and ⌘[ act on the frame they arrive).
struct FirstStartCardShell: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let still = reduceMotion || model.frozen
        let step = min(model.step, 5)
        let rect = geo.card(step: step)
        let radius: CGFloat = step == 5 ? 16 : (step == 1 ? 18 : 14)
        let shape = RoundedRectangle(cornerRadius: radius, style: .continuous)
        ZStack {
            FirstStartCardContent(model: model, geo: geo, step: step).id(step)
                .transition(.opacity.animation(.easeOut(duration: 0.14)))
        }
        .frame(width: rect.width, height: rect.height)
        .background(shape.fill(FirstStartLook.cardTint.color(scheme)))
        .whooshGlass(in: shape)
        .whooshAx("\(FirstStartFeature.X).card")   // on the card itself: its frame is the card's (it was the whole window's, so a check of the card read the mail around it; main170, welcome-ai-step)
        .shadow(color: .black.opacity(scheme == .dark ? 0.45 : 0.14), radius: 24, y: 12)
        .firstStartEntrance(from: 0.98, rise: 12, duration: FirstStartLook.morphS, delay: 0.04, still: model.frozen)   // 3a: the card rises 12 pt from 0.98 in the ledger's 200 ms, from 40 ms
        .offset(x: rect.minX, y: rect.minY)
        .animation(WhooshMotion.animation(.timingCurve(0.22, 1, 0.36, 1, duration: FirstStartLook.morphS), reduceMotion: still), value: step)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

struct FirstStartCardContent: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    let step: Int

    var body: some View {
        let t = FirstStartCardText.of(step)
        if step == 1 { welcome(t) } else { coach(t) }
    }

    // 3a: the centred welcome card, crowned by the medallion (drawn by the art layer)
    private func welcome(_ t: FirstStartCardText.Copy) -> some View {
        VStack(spacing: 0) {
            Text(t.kicker).font(.system(size: 12, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
            Text(verbatim: t.title).font(.system(size: 30, weight: .bold)).tracking(-0.8).foregroundStyle(WhooshPalette.label).padding(.top, 2).padding(.bottom, 8)
            Text(t.body).font(.system(size: 13.5)).lineSpacing(3).multilineTextAlignment(.center).foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(maxWidth: 318).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
            FirstStartDots(current: 1, pill: true, still: model.frozen).padding(.bottom, 14)
            HStack(spacing: 8) {
                Text(FirstStartWords.stepOf(1)).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.tertiaryLabel)
                Spacer(minLength: 0)
                FirstStartButton(id: "skip", title: FirstStartWords.skip, key: "esc") { _ = model.skip() }
                FirstStartButton(id: "continue", title: FirstStartWords.continueWord, key: "↩", primary: true) { _ = model.next() }
            }
        }
        .padding(.horizontal, 22).padding(.bottom, 16).padding(.top, geo.medallion / 2 + 22)
    }

    // 2b: kicker, title, sentence, the step's own extra, dots, then Back / Skip / Continue
    private func coach(_ t: FirstStartCardText.Copy) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 6) { kickerIcon; Text(t.kicker) }
                .font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
            Text(t.title).font(.system(size: step == 5 ? 20 : 19, weight: .semibold)).tracking(-0.35).lineSpacing(2)
                .foregroundStyle(WhooshPalette.label).padding(.top, 6).padding(.bottom, 6)
            sentence(t)
            extra.padding(.top, 12)
            Spacer(minLength: 0)
            HStack {
                FirstStartDots(current: step, still: model.frozen)
                Spacer(minLength: 0)
                Text(FirstStartWords.stepOf(step)).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.tertiaryLabel)
            }.padding(.top, 14).padding(.bottom, 12)
            HStack(spacing: 8) {
                FirstStartButton(id: "back", title: FirstStartWords.back, key: "⌘[") { _ = model.back() }
                Spacer(minLength: 0)
                FirstStartButton(id: "skip", title: FirstStartWords.skip, key: "esc") { _ = model.skip() }
                FirstStartButton(id: "continue", title: FirstStartWords.continueWord, key: "↩", primary: true) { _ = model.next() }
            }
        }
        .padding(.horizontal, 20).padding(.top, 18).padding(.bottom, 16)
    }

    @ViewBuilder private func sentence(_ t: FirstStartCardText.Copy) -> some View {
        if step == 2 {
            (Text(t.body) + Text(" ") + Text(FirstStartWords.tryIt).fontWeight(.semibold).foregroundStyle(WhooshPalette.label))
                .font(.system(size: 13)).lineSpacing(3).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
        } else {
            Text(t.body).font(.system(size: 13)).lineSpacing(3).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
        }
    }

    @ViewBuilder private var kickerIcon: some View {
        switch step {
        case 2: Image(systemName: "magnifyingglass").font(.system(size: 11, weight: .semibold))
        case 4: Image(systemName: "tag").font(.system(size: 11, weight: .semibold))
        case 5: Image(systemName: "lock").font(.system(size: 11, weight: .semibold))
        default: EmptyView()
        }
    }

    @ViewBuilder private var extra: some View {
        switch step {
        case 2:
            HStack(spacing: 6) {
                FirstStartKbd(text: "⌘K"); Text(FirstStartWords.searchOr); FirstStartKbd(text: "/"); Text(FirstStartWords.searchEverywhere)
            }.font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel)
        case 4: FirstStartLegend()
        case 5: FirstStartPrivacyRows()
        default: EmptyView()
        }
    }
}

/// Step 4's legend: 1 Your rules, 2 The model on this Mac (2b `.legend`).
struct FirstStartLegend: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            row(1, FirstStartWords.yourRules, FirstStartWords.youWriteThem)
            row(2, FirstStartWords.modelOnMac, FirstStartWords.whenNoRule)
        }
    }
    private func row(_ n: Int, _ title: String, _ small: String) -> some View {
        HStack(spacing: 10) {
            Text("\(n)").font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(width: 20, height: 20).background(WhooshPalette.fill, in: Circle())
            (Text(title).foregroundStyle(WhooshPalette.label) + Text(" · ") + Text(small))
                .font(.system(size: 12.5)).foregroundStyle(WhooshPalette.secondaryLabel)
        }
    }
}

/// Step 5's two optional parts, each with its switch off, then the line that nothing goes to Microsoft or Google (2b `.orow`, `.never`).
struct FirstStartPrivacyRows: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            optional("key", FirstStartWords.jevKey, FirstStartWords.jevHow)
            optional("iphone", FirstStartWords.pushRelay, FirstStartWords.pushHow)
            HStack(spacing: 8) {
                Image(systemName: "checkmark.shield").font(.system(size: 13)).foregroundStyle(WhooshPalette.accent)
                Text(FirstStartWords.nothingSent).font(.system(size: 12.5)).foregroundStyle(WhooshPalette.label)
            }.padding(.top, 9).overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: 0.5) }
        }
    }
    private func optional(_ icon: String, _ title: String, _ how: String) -> some View {
        HStack(spacing: 12) {
            Image(systemName: icon).font(.system(size: 13)).foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(width: 28, height: 28).background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            VStack(alignment: .leading, spacing: 0) {
                Text(title).font(.system(size: 13, weight: .semibold)).foregroundStyle(WhooshPalette.label)
                Text(how).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel)
            }
            Spacer(minLength: 0)
            FirstStartOffSwitch()
        }
        .padding(.vertical, 8).overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: 0.5) }
    }
}

/// A switch that is off and shows it: 32 x 19, the knob left (2b `.sw`). It is a picture of the setting, never a control here.
struct FirstStartOffSwitch: View {
    var body: some View {
        Capsule().fill(Color.gray.opacity(0.24)).frame(width: 32, height: 19)
            .overlay(alignment: .leading) { Circle().fill(Color.white).frame(width: 15, height: 15).shadow(color: .black.opacity(0.25), radius: 1, y: 1).padding(.leading, 2) }
            .accessibilityHidden(true)
    }
}
