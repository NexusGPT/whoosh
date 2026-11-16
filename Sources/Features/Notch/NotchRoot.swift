import SwiftUI
import UniformTypeIdentifiers

/// The whole notch, look A "Continuum" (docs/design/boards/notch/a): one black silhouette that IS the camera housing.
/// At rest with unread mail it grows two wings beside the camera (the newest unread mail's account, the unread count);
/// an arrival grows it down into the card, the pointer into the list, a click into the open mail; Escape, the close
/// mark or leaving shrinks it back. The growth is the one signature move: the system spring without bounce,
/// MOTION_MAX_MS long, retargeted by the next input (never queued). Content inside only cross-fades.
struct NotchRoot: View {
    let geometry: NotchGeometry
    var model = NotchModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.colorScheme) private var scheme
    @State private var dropTargeted = false

    static var growth: Animation { .smooth(duration: Double(WhooshConstants.MOTION_MAX_MS) / 1000) }
    /// The growth spring, except for an arrival: the drop is full width on the arrival frame and only its card fades and
    /// rises (NotchEntrance), so the shape's frame never animates while a mail lands.
    private var motion: Animation? { model.stage == .drop ? nil : WhooshMotion.animation(Self.growth, reduceMotion: reduceMotion) }
    /// The card is out under the band: an arrival's card, the list, an open mail. A light and a line keep the housing's height.
    private var grown: Bool { switch model.stage { case .rest, .light, .line: return false; case .drop, .list, .open: return true } }
    private var winged: Bool { model.wingsShown }
    private var lined: Bool { model.stage == .line }

    /// The shape's width: the housing alone, the housing plus two wings, or the stage's card (board a: 460 / 480 / 540).
    private var shapeWidth: CGFloat {
        switch model.stage {
        case .rest, .light: return geometry.housing.width + (winged ? 2 * NotchMetrics.restWing : 0)
        case .line: return NotchMetrics.lineWidth
        case .open: return NotchMetrics.openWidth
        case .drop: return NotchMetrics.cardWidth
        case .list: return NotchMetrics.listWidth
        }
    }
    private var shoulder: CGFloat { grown ? NotchMetrics.shoulder : (winged || lined ? NotchMetrics.restShoulder : 0) }
    private var corner: CGFloat { grown ? NotchMetrics.openCorner : NotchMetrics.restCorner }
    private var silhouette: NotchSilhouette { NotchSilhouette(shoulder: shoulder, corner: corner) }

    var body: some View {
        VStack(spacing: 0) {
            NotchBand(model: model, housing: geometry.housing, shapeWidth: shapeWidth)
                .frame(width: shapeWidth, height: geometry.housing.height)
            if grown {
                NotchCard(model: model)
                    .modifier(NotchEntrance())
                    .padding(.horizontal, NotchMetrics.bodyInset)
                    .padding(.top, NotchMetrics.bodyTop)
                    .padding(.bottom, NotchMetrics.bodyBottom)
                    .transition(.opacity)
            }
        }
        .frame(width: shapeWidth)
        .padding(.horizontal, shoulder)
        .background { Color.black }
        .clipShape(silhouette)
        .overlay(alignment: .bottom) {
            // a new mail as a light: the lower edge in the account's colour, outside the clip so its glow reaches the desktop
            if model.stage == .light, let it = model.dropItem {
                NotchLight(colour: AccountColour.ring(RowAccount.of(it.accountId, in: AppModel.shared).colour), unread: model.unreadCount)
                    .padding(.horizontal, shoulder + NotchMetrics.restCorner)
                    .transition(.opacity)
            }
        }
        .overlay {
            // a hairline of the dark separator inside the lower edge, so the black reads as an object on a black desktop;
            // masked below the band, so the top edge meets the bezel with no seam
            silhouette.stroke(WhooshPalette.separator, lineWidth: WhooshMetrics.separator)
                .mask { VStack(spacing: 0) { Color.clear.frame(height: geometry.housing.height); Color.black } }
                .opacity(grown || winged || lined ? 1 : 0)
        }
        .overlay {
            silhouette.stroke(dropTargeted ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(Color.clear), lineWidth: WhooshMetrics.separator * 2)
        }
        .shadow(color: grown || winged || lined ? WhooshGlass.shadow.color(scheme) : .clear, radius: WhooshSpacing.s16, y: WhooshSpacing.s6)
        .whooshAx(NotchIds.shape)
        .onGeometryChange(for: CGSize.self) { $0.size } action: { model.shapeSize = $0 }
        .onDrop(of: [.fileURL], isTargeted: $dropTargeted) { providers in
            guard let p = providers.first else { return false }
            _ = p.loadObject(ofClass: URL.self) { url, _ in
                guard let url else { return }
                DispatchQueue.main.async { MainActor.assumeIsolated { _ = NotchModel.shared.dropFile(url) } }
            }
            return true
        }
        #if os(macOS)
        .onExitCommand { model.collapse() }
        #endif
        .task(id: model.dropToken) {
            // Off the input path: one sleep per drop, nothing waits on it (docs/MOTION.md rule 10).
            guard model.stage == .drop || model.stage == .light || model.stage == .line else { return }
            // how long an alert stays is the owner's setting (5 s, 10 s, until I act); "until I act" starts no timer
            guard let seconds = notchStaySeconds(stay: model.alerts.stay) else { return }
            let token = model.dropToken
            do { try await Task.sleep(for: .seconds(Int(seconds))) } catch { return }
            model.holdEnded(token)
        }
        .animation(motion, value: model.stage)
        .animation(motion, value: model.listed.count)
        .animation(motion, value: model.notice)
        .animation(motion, value: model.wingsShown)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        // the tips layer spans the whole canvas, outside the silhouette's clip (NotchTips.swift)
        .coordinateSpace(.named(NotchTips.space))
        .overlayPreferenceValue(NotchTipsKey.self) { NotchTipsLayer(tips: $0) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Words.text("Whoosh notch"))
    }
}

/// A new mail as a light (owner's picks of 14 Nov 2026, alert style "Light"): a thin line in the account's colour on the notch's lower edge, with its
/// glow. No words; the unread count beside the camera says the rest.
struct NotchLight: View {
    let colour: Color
    let unread: Int

    var body: some View {
        Capsule()
            .fill(colour)
            .frame(height: NotchMetrics.lightHeight)
            .shadow(color: colour.opacity(0.9), radius: WhooshSpacing.s6)
            .whooshAx(NotchIds.light)
            .accessibilityLabel(NotchCopy.unreadCount(unread))
    }
}
