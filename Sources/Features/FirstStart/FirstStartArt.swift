import SwiftUI

/// The art behind the card (z below it): the halo of step 1, the aurora stage of the lit pane on steps 2 to 5, the halo behind the sheet
/// on steps 6 and 7 with the sidebar's account slot. Nothing here takes a click except the step-2 field.
struct FirstStartBackArt: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry

    var body: some View {
        ZStack(alignment: .topLeading) {
            if model.active && !model.skipped { art }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    @ViewBuilder private var art: some View {
        switch model.step {
        case 1:
            FirstStartClock(model: model, step: 1) { t in
                FirstStartHalo(t: t, size: geo.haloSize, inner: geo.medallion * 0.6)
                    .firstStartEntrance(from: 0.55, duration: FirstStartLook.haloS, delay: FirstStartLook.haloDelayS, still: model.frozen)   // 3a: the halo opens from 0.55 in 420 ms, from 130 ms
                    .position(x: geo.size.width / 2, y: geo.welcomeCard.minY)
            }
        case 2: FirstStartStage(model: model, geo: geo, pane: "list") { t in FirstStartSearchArt(model: model, geo: geo, t: t) }
        case 3: FirstStartStage(model: model, geo: geo, pane: "list") { t in FirstStartKeysArt(model: model, geo: geo, t: t) }
        case 4:
            FirstStartStage(model: model, geo: geo, pane: "list") { t in FirstStartLabelsArt(geo: geo, t: t) }
            FirstStartClock(model: model, step: 4) { t in FirstStartSidebarLights(geo: geo, t: t) }
        case 5: FirstStartStage(model: model, geo: geo, pane: "reader") { t in FirstStartPrivacyArt(geo: geo, t: t) }
        case FirstStartModel.aiStep: FirstStartStage(model: model, geo: geo, pane: "list") { t in FirstStartAIArt(model: model, geo: geo, t: t) }
        case FirstStartModel.repliesStep: FirstStartStage(model: model, geo: geo, pane: "list") { t in FirstStartRepliesArt(model: model, geo: geo, t: t) }
        default: FirstStartSheetHalo(model: model, geo: geo)
        }
    }
}

/// The lit pane as a stage (2b): the aurora ground below the pane's header, the art on it. Art coordinates are the board's: the pane's
/// left edge, the window's top, so every number below is read straight off the board.
struct FirstStartStage<Art: View>: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    let pane: String
    @ViewBuilder let art: (Double) -> Art

    var body: some View {
        let r = geo.pane(pane)
        let top = pane == "list" ? FirstStartGeometry.stageTop : FirstStartGeometry.toolbar
        FirstStartClock(model: model, step: model.step) { t in
            ZStack(alignment: .topLeading) {
                Rectangle().fill(WhooshPalette.background)   // the stage replaces what the pane holds: the aurora ground is not opaque, so the list's rows showed through it once an account's mail was there (main105 step 7)
                    .frame(width: r.width, height: geo.size.height - top)
                    .offset(y: top)
                FirstStartAuroraGround(t: t)
                    .frame(width: r.width, height: geo.size.height - top)
                    .offset(y: top)
                art(t)
            }
            .frame(width: r.width, height: geo.size.height, alignment: .topLeading)
            .clipped()
            .offset(x: r.minX)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
    }
}

/// Steps 6 and 9: an aurora halo breathes behind the sheet, and the sidebar's account slot shows where the account will appear (2b `'6'`, `'7'`).
struct FirstStartSheetHalo: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let cx = geo.size.width / 2, cy = geo.size.height / 2 - 18
        let w: CGFloat = model.step == FirstStartModel.addStep ? 1120 : 1200, h: CGFloat = model.step == FirstStartModel.addStep ? 660 : 680
        ZStack(alignment: .topLeading) {
            FirstStartClock(model: model, step: model.step) { t in
                FirstStartAuroraGround(t: t)
                    .frame(width: w, height: h)
                    .mask(RadialGradient(stops: [.init(color: .black, location: 0.55), .init(color: .clear, location: 1)], center: .center, startRadius: 0, endRadius: min(w, h) / 2))
                    .opacity(0.9)
                    .position(x: cx, y: cy)
            }
            slot
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .allowsHitTesting(false)
    }

    /// 2b `.slot`: 42 pt tall at the sidebar's foot; step 9 shows the new account fetching its history.
    private var slot: some View {
        let w = geo.sidebarW - 16
        return HStack(spacing: 10) {
            Image(systemName: model.step == FirstStartModel.addStep ? "plus" : "icloud").font(.system(size: 11))
                .foregroundStyle(model.step == FirstStartModel.addStep ? WhooshPalette.secondaryLabel : WhooshPalette.accent)
                .frame(width: 22, height: 22).background(WhooshPalette.fill, in: Circle())
            VStack(alignment: .leading, spacing: 0) {
                Text(model.step == FirstStartModel.addStep ? FirstStartWords.yourAccount : accountName)
                    .font(.system(size: 12.5, weight: .semibold)).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Text(model.step == FirstStartModel.addStep ? FirstStartWords.appearsHere : FirstStartWords.fetching)
                    .font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12).frame(width: w, height: 42)
        .background(WhooshPalette.background, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .shadow(color: .black.opacity(0.12), radius: 10, y: 4)
        .offset(x: 8, y: geo.size.height - 50)
    }

    private var accountName: String { CoreHost.shared.core.accounts().last.map { AccountNames.display($0) } ?? FirstStartWords.yourAccount }   // the name through AccountNames: an imported account reads in the app's language
}

/// The art in front of the card (z above it): the medallion that crowns the welcome card (3a). The card's text sits under it.
struct FirstStartFrontArt: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry

    var body: some View {
        ZStack(alignment: .topLeading) {
            if model.active && !model.skipped && model.step == 1 {
                FirstStartClock(model: model, step: 1) { t in
                    FirstStartMedallion(t: t, size: geo.medallion, still: model.frozen)
                        .offset(x: geo.medallionOrigin.x, y: geo.medallionOrigin.y)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .allowsHitTesting(false)
    }
}
