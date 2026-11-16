import SwiftUI

/// The shader's arguments, in the order of `sendWave` in SendWaveShaderSource.
enum SendWaveShader {
    static let library: ShaderLibrary = ShaderLibrary(data: SendWaveShaderLibrary.metallib)

    /// The farthest a sample travels: the lens with red's extra share, the edge pull, and the edge probe's 4 pt.
    static var maxOffset: CGSize {
        let d = SendWaveTuning.lensPt * (1 + SendWaveTuning.channelOffset) + SendWaveTuning.lensPt * 0.6 + 5
        return CGSize(width: d, height: d)
    }

    @MainActor private static var warmed = false
    /// Compiles the pipeline once, when the first pane that can wave appears, so the first send pays nothing.
    @MainActor static func warm() async {
        guard !warmed else { return }
        warmed = true
        try? await make(t: 0, size: CGSize(width: 1, height: 1), origin: .zero, dark: false).compile(as: .layerEffect)
    }

    /// `reach` is the one wave's: from its origin to the farthest edge of every window it crosses (so each window
    /// draws its slice of the same ring at the same moment); nil measures this drawing alone.
    static func make(t: Double, size: CGSize, origin: CGPoint, hole: CGRect = .zero, dark: Bool, reach: CGFloat? = nil,
                     band: Double = SendWaveTuning.band, fadeFrom: Double = SendWaveTuning.fadeFrom) -> Shader {
        let corners = [CGPoint.zero, CGPoint(x: size.width, y: 0), CGPoint(x: 0, y: size.height), CGPoint(x: size.width, y: size.height)]
        let reach = reach ?? (corners.map { hypot($0.x - origin.x, $0.y - origin.y) }.max() ?? 1)
        let scheme: ColorScheme = dark ? .dark : .light
        let rings = SendWaveScheme.current.rings.map { Shader.Argument.color($0.color(scheme)) }
        let fn = ShaderFunction(library: library, name: SendWaveShaderSource.function)
        return Shader(function: fn, arguments: [
            .float2(size), .float2(origin), .float(t), .float(reach), .float(dark ? 1 : 0),
        ] + rings + [
            .float(SendWaveTuning.channelOffset), .float(SendWaveTuning.lensPt), .float(band),
            .float(dark ? SendWaveTuning.strengthDark : SendWaveTuning.strengthLight),
            .float(dark ? SendWaveTuning.edgeDark : SendWaveTuning.edgeLight),
            .float4(Float(hole.minX), Float(hole.minY), Float(hole.width), Float(hole.height)),
            .float(fadeFrom),
        ])
    }
}
