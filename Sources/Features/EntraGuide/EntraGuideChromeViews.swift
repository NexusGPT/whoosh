import SwiftUI

/// The head: the Microsoft mark in its glass chip, the title, the chapter and "Step N of M", and Close.
struct EntraGuideHeadView: View {
    let model: EntraGuideModel
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        HStack(spacing: WhooshSpacing.s12) {
            MarkChip(kind: .microsoft, diameter: 36)
            VStack(alignment: .leading, spacing: 0) {
                Text(EntraGuideWords.title).whooshText(WhooshTypography.title3).foregroundStyle(WhooshPalette.label)
                HStack(spacing: 6) {
                    Text(model.step.chapterTitle).foregroundStyle(WhooshPalette.label)
                    Text("·")
                    Text(EntraGuideWords.stepOf(model.number, model.total))
                }
                .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                .id(model.index).transition(.opacity)
                .accessibilityIdentifier("\(EntraGuideWords.surface).count")
            }
            Spacer(minLength: 8)
            EntraGuideButton(id: "close", title: EntraGuideWords.close, look: .plain) { _ = model.close() }
        }
        .animation(EntraGuideMetrics.motion(.easeInOut(duration: EntraGuideMetrics.fadeS), reduce), value: model.index)
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(EntraGuideWords.surface).head")
    }
}

/// One hairline segment per step (the owner's amendment: every click is its own segment). The current segment and all before it are
/// filled; on a valid ID at the last step the whole bar turns into the aurora colours once (260 ms).
struct EntraGuideProgressView: View {
    let model: EntraGuideModel
    @Environment(\.accessibilityReduceMotion) private var reduce
    @Environment(\.colorScheme) private var scheme

    private var aurora: Bool { model.isLast && model.validId }

    var body: some View {
        ZStack {
            segments(WhooshPalette.accent)
            LinearGradient(colors: WhooshPalette.guideAurora.map { $0.color(scheme) },
                           startPoint: .leading, endPoint: .trailing)
                .mask(segments(Color.black, clearTrack: true)).opacity(aurora ? 1 : 0)
        }
        .frame(height: 2)
        .animation(EntraGuideMetrics.motion(.easeOut(duration: EntraGuideMetrics.auroraS), reduce), value: aurora)
        .accessibilityElement(children: .ignore).accessibilityIdentifier("\(EntraGuideWords.surface).progress")
    }

    private func segments<S: ShapeStyle>(_ fill: S, clearTrack: Bool = false) -> some View {
        HStack(spacing: EntraGuideMetrics.segmentGap) {
            ForEach(0..<max(model.total, 1), id: \.self) { i in
                ZStack(alignment: .leading) {
                    if !clearTrack { Rectangle().fill(WhooshPalette.separator) }
                    Rectangle().fill(fill).scaleEffect(x: i <= model.index ? 1 : 0, anchor: .leading)
                }
                .clipShape(RoundedRectangle(cornerRadius: 1))
            }
        }
        .animation(EntraGuideMetrics.motion(.easeOut(duration: EntraGuideMetrics.stepHalfS), reduce), value: model.index)
    }
}

/// The browser-bar line over the plate: it names where in Entra the person is.
struct EntraGuideWhereBar: View {
    let location: String
    var body: some View {
        HStack(spacing: 8) {
            HStack(spacing: 6) { ForEach(0..<3, id: \.self) { _ in Circle().fill(WhooshPalette.guideChromeDot).frame(width: 9, height: 9) } }.padding(.trailing, 6)
            Image(systemName: "lock.fill").font(.system(size: 9)).foregroundStyle(WhooshPalette.guideChromeLock)
            Text(EntraGuideWords.entraHost).font(.system(size: 11.5, weight: .semibold)).foregroundStyle(WhooshPalette.guideChromeHost)
            Text("›  " + location).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.guideChromeLocation).lineLimit(1).truncationMode(.head)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12).frame(height: EntraGuideMetrics.whereBar).background(WhooshPalette.guideChromeBar)
        .overlay(alignment: .bottom) { Rectangle().fill(WhooshPalette.guideChromeEdge).frame(height: 1) }
        .accessibilityIdentifier("\(EntraGuideWords.surface).where")
    }
}

/// The foot: Skip to paste at the left; the opened note, Back and Next (Save and sign in at the end) at the right.
struct EntraGuideFooterView: View {
    let model: EntraGuideModel
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        HStack(spacing: 8) {
            if !model.isLast { EntraGuideButton(id: "skip", title: EntraGuideWords.skip, look: .plain) { _ = model.skip() }.transition(.opacity) }
            Spacer(minLength: 8)
            if model.step.primary == true && model.opened {
                Label(EntraGuideWords.opened, systemImage: "checkmark").whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.guideOK)
                    .padding(.trailing, 4).transition(.opacity).accessibilityIdentifier("\(EntraGuideWords.surface).opened")
            }
            EntraGuideButton(id: "back", title: EntraGuideWords.back, enabled: model.index > 0) { _ = model.back() }
            EntraGuideButton(id: model.isLast ? "save" : "next", title: nextTitle, look: .primary, minWidth: 112, enabled: !model.isLast || model.validId) { _ = model.next() }
        }
        .animation(EntraGuideMetrics.motion(.easeInOut(duration: EntraGuideMetrics.fadeS), reduce), value: nextTitle)
        .animation(EntraGuideMetrics.motion(.easeInOut(duration: EntraGuideMetrics.fadeS), reduce), value: model.isLast)
        .animation(EntraGuideMetrics.motion(.easeInOut(duration: EntraGuideMetrics.fadeS), reduce), value: model.opened)
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(EntraGuideWords.surface).footer")
    }

    var nextTitle: String { model.isLast ? EntraGuideWords.save : (model.nextIsOpen ? EntraGuideWords.openEntra : EntraGuideWords.next) }
}
