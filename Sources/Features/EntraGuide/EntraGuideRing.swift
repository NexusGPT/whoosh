import SwiftUI

/// The glass ring on the control to click. It enters at scale 1.10 to 1 in 220 ms, then one halo pulse of 260 ms; it never loops.
/// A short move on the same picture slides the ring instead (the parent keeps its identity and animates `box`).
struct EntraGuideRingView: View {
    let box: CGRect
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var shown = false
    /// 0 hidden, 1 the halo at its start, 2 the halo spent
    @State private var pulse = 0

    var body: some View {
        RoundedRectangle(cornerRadius: 9, style: .continuous)
            .strokeBorder(WhooshPalette.accent, lineWidth: 2)
            .background(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(WhooshPalette.focusHalo, lineWidth: 4).padding(-2).opacity(0.5))
            .shadow(color: .black.opacity(0.12), radius: 5, y: 2)
            .overlay {
                RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(WhooshPalette.accent, lineWidth: 2)
                    .scaleEffect(pulse == 2 ? 1.22 : 1).opacity(pulse == 1 ? 0.6 : 0)
            }
            .frame(width: max(box.width, 0), height: max(box.height, 0))
            .scaleEffect(shown ? 1 : 1.10).opacity(shown ? 1 : 0)
            .offset(x: box.minX, y: box.minY)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .allowsHitTesting(false)
            .onAppear {
                withAnimation(EntraGuideMetrics.motion(EntraGuideMetrics.ease(EntraGuideMetrics.ringInS), reduce)) { shown = true }
                Task { @MainActor in
                    try? await Task.sleep(nanoseconds: UInt64(EntraGuideMetrics.ringInS * 1_000_000_000))
                    pulse = 1
                    try? await Task.sleep(nanoseconds: 16_000_000)
                    withAnimation(EntraGuideMetrics.motion(.easeOut(duration: EntraGuideMetrics.haloS), reduce)) { pulse = 2 }
                }
            }
            .accessibilityIdentifier("\(EntraGuideWords.surface).ring")
    }
}

/// The caption sits on the ring's top edge like a tab, so it covers the page as little as possible.
struct EntraGuideCaptionView: View {
    let text: String
    let ring: CGRect
    let plate: CGSize
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var shown = false

    var body: some View {
        let x = max(8, min(ring.minX + 10, plate.width - 320))
        let y = ring.minY > 16 ? ring.minY - 13 : min(plate.height - 30, ring.maxY - 13)
        Text(text).font(.system(size: 12, weight: .semibold)).foregroundStyle(WhooshPalette.onAccent).lineLimit(1).truncationMode(.tail)
            .padding(.horizontal, 10).padding(.vertical, 4).frame(maxWidth: 300, alignment: .leading)
            .background(WhooshPalette.accent, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .shadow(color: .black.opacity(0.18), radius: 4, y: 2)
            .offset(x: x, y: y + (shown ? 0 : -4))
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .opacity(shown && !text.isEmpty ? 1 : 0)
            .allowsHitTesting(false)
            .onAppear {
                withAnimation(EntraGuideMetrics.motion(EntraGuideMetrics.ease(EntraGuideMetrics.capInS).delay(EntraGuideMetrics.capDelayS), reduce)) { shown = true }
            }
            .accessibilityIdentifier("\(EntraGuideWords.surface).caption")
    }
}
