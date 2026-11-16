import ImageIO
import SwiftUI

/// One provider's mark at the Settings pane-icon size, under the id `mac-settings-ai.provider.<slug>.mark`. The mark is the
/// vendor's own vector data (ProviderMarks.swift); a vendor whose asset is not in the tree draws nothing, never a stand-in,
/// and the probe `rows mac-settings-ai.logos` names it (`present: false`, with the path the asset is expected at).
/// `probe: false` is for a mark repeated on one screen (an agent card), where one id would name several views.
struct ProviderMarkView: View {
    let mark: ProviderMark
    var side: CGFloat = SettingsMetrics.paneIcon
    var probe = true

    var body: some View {
        tagged(art.frame(width: side, height: side).accessibilityHidden(true))
    }

    @ViewBuilder private var art: some View {
        if mark == .gemini {
            GeminiMark()
        } else if let a = mark.art {
            ZStack {
                ForEach(a.paths.indices, id: \.self) { i in
                    SVGPathShape(d: a.paths[i].d, viewBox: a.viewBox).fill(a.paths[i].fill, style: FillStyle(eoFill: a.paths[i].evenOdd))
                }
            }
        } else {
            Color.clear
        }
    }

    @ViewBuilder private func tagged<V: View>(_ v: V) -> some View {
        if probe && mark.present { v.whooshAx("mac-settings-ai.provider.\(mark.rawValue).mark") } else { v }
    }
}

/// Gemini's sparkle as Google serves it: its photographic gradient clipped to the sparkle's path (Marks/gemini.svg).
@MainActor
private struct GeminiMark: View {
    static let viewBox = CGSize(width: 192, height: 192)
    static let sparkle = "M164.93 86.68c-13.56-5.84-25.42-13.84-35.6-24.01-10.17-10.17-18.18-22.04-24.01-35.6-2.23-5.19-4.04-10.54-5.42-16.02C99.45 9.26 97.85 8 96 8s-3.45 1.26-3.9 3.05c-1.38 5.48-3.18 10.81-5.42 16.02-5.84 13.56-13.84 25.43-24.01 35.6-10.17 10.16-22.04 18.17-35.6 24.01-5.19 2.23-10.54 4.04-16.02 5.42C9.26 92.55 8 94.15 8 96s1.26 3.45 3.05 3.9c5.48 1.38 10.81 3.18 16.02 5.42 13.56 5.84 25.42 13.84 35.6 24.01 10.17 10.17 18.18 22.04 24.01 35.6 2.24 5.2 4.04 10.54 5.42 16.02A4.03 4.03 0 0 0 96 184c1.85 0 3.45-1.26 3.9-3.05 1.38-5.48 3.18-10.81 5.42-16.02 5.84-13.56 13.84-25.42 24.01-35.6 10.17-10.17 22.04-18.18 35.6-24.01 5.2-2.24 10.54-4.04 16.02-5.42A4.03 4.03 0 0 0 184 96c0-1.85-1.26-3.45-3.05-3.9-5.48-1.38-10.81-3.18-16.02-5.42"
    static let image: CGImage? = {
        guard let data = Data(base64Encoded: GeminiMarkImage.jpeg), let src = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        return CGImageSourceCreateImageAtIndex(src, 0, nil)
    }()
    var body: some View {
        GeometryReader { g in
            let k = g.size.width / Self.viewBox.width
            if let img = Self.image {
                Image(decorative: img, scale: 1).resizable()
                    .frame(width: 472 * k, height: 461 * k)
                    .offset(x: -187.2 * k, y: -95.81 * k)
                    .frame(width: g.size.width, height: g.size.height, alignment: .topLeading)
                    .clipShape(SVGPathShape(d: Self.sparkle, viewBox: Self.viewBox))
            }
        }
    }
}

