import SwiftUI
import ImageIO

/// A camera over one picture: `translate(tx, ty) scale(k)` in points, the board's `fit`. The picture's own pixels are the unit, so a
/// ring or focus box is a fraction of the picture (x0, y0, x1, y1) and means the same control at any window size.
struct EntraGuideCamera: Equatable {
    var k: Double, tx: Double, ty: Double

    /// Zoom so the focus box fills the plate, centred; never show empty plate beside a picture big enough to fill it.
    static func fit(w: Double, h: Double, focus f: [Double], in size: CGSize) -> EntraGuideCamera {
        let pw = Double(size.width), ph = Double(size.height)
        guard f.count == 4, w > 0, h > 0, pw > 0, ph > 0 else { return EntraGuideCamera(k: 1, tx: 0, ty: 0) }
        let fw = (f[2] - f[0]) * w, fh = (f[3] - f[1]) * h
        let k = min(pw / fw, ph / fh)
        var tx = pw / 2 - (f[0] + f[2]) / 2 * w * k, ty = ph / 2 - (f[1] + f[3]) / 2 * h * k
        let iw = w * k, ih = h * k
        tx = iw >= pw ? min(0, max(pw - iw, tx)) : (pw - iw) / 2
        if ih >= ph { ty = min(0, max(ph - ih, ty)) }
        return EntraGuideCamera(k: k, tx: tx, ty: ty)
    }

    /// The ring's box on the plate, a few points wider than the control on every side.
    func box(ring r: [Double], w: Double, h: Double, pad: Double) -> CGRect {
        guard r.count == 4 else { return .zero }
        return CGRect(x: tx + r[0] * w * k - pad, y: ty + r[1] * h * k - pad, width: (r[2] - r[0]) * w * k + 2 * pad, height: (r[3] - r[1]) * h * k + 2 * pad)
    }
}

/// The pictures, decoded once at full size off the main thread, so no step waits on a decode. Only the few nearest are kept.
@MainActor
final class EntraGuideImages {
    static let shared = EntraGuideImages()
    private var cache: [String: CGImage] = [:]
    private var order: [String] = []
    private var pending: Set<String> = []

    func image(_ file: String) -> CGImage? { cache[file] ?? Self.decode(file).map { store(file, $0) } }

    /// Starts decoding the files in the background; the cache keeps the newest few.
    func warm(_ files: [String]) {
        for f in files where cache[f] == nil && !pending.contains(f) {
            pending.insert(f)
            Task.detached(priority: .userInitiated) {
                let img = EntraGuideImages.decode(f)
                await MainActor.run {
                    EntraGuideImages.shared.pending.remove(f)
                    if let img { _ = EntraGuideImages.shared.store(f, img) }
                }
            }
        }
    }

    @discardableResult
    private func store(_ file: String, _ img: CGImage) -> CGImage {
        cache[file] = img; order.removeAll { $0 == file }; order.append(file)
        while order.count > 5 { cache[order.removeFirst()] = nil }
        return img
    }

    nonisolated private static func decode(_ file: String) -> CGImage? {
        guard let u = EntraGuideData.url(file, "png"), let src = CGImageSourceCreateWithURL(u as CFURL, nil) else { return nil }
        return CGImageSourceCreateImageAtIndex(src, 0, [kCGImageSourceShouldCacheImmediately: true] as CFDictionary)
    }
}
