#if os(macOS)
import AppKit
import Foundation

/// Where an uploaded account image lives: under the app container, one square PNG per account (docs/CONSTANTS.md
/// ACCOUNT_ICON_PX stored at 2x, ACCOUNT_ICON_MAX_BYTES upload limit). Nothing leaves this Mac.
enum AccountIconStore {
    static var folder: URL {
        let base = ProcessInfo.processInfo.environment["WHOOSH_ICON_DIR"].map { URL(fileURLWithPath: $0) }
            ?? ModelsFolder.whooshRoot().appendingPathComponent("AccountIcons")
        return base
    }

    /// nil when the size is allowed; otherwise the reason in words. At the limit is allowed, one byte over is not.
    static func verdict(byteCount: Int) -> String? {
        byteCount > WhooshConstants.ACCOUNT_ICON_MAX_BYTES ? AccountsWords.tooBig(limit: WhooshConstants.ACCOUNT_ICON_MAX_BYTES) : nil
    }

    /// Cuts the picture to a centred square at ACCOUNT_ICON_PX x 2 and keeps it. Returns the file path or the reason.
    static func store(_ data: Data, accountId: String) -> Result<String, StoreError> {
        if let why = verdict(byteCount: data.count) { return .failure(.words(why)) }
        guard let src = NSBitmapImageRep(data: data), let cg = src.cgImage else { return .failure(.words(AccountsWords.notAnImage)) }
        let side = min(cg.width, cg.height)
        let crop = CGRect(x: (cg.width - side) / 2, y: (cg.height - side) / 2, width: side, height: side)
        guard let square = cg.cropping(to: crop) else { return .failure(.words(AccountsWords.notAnImage)) }
        let px = WhooshConstants.ACCOUNT_ICON_PX * 2
        guard let ctx = CGContext(data: nil, width: px, height: px, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return .failure(.words(AccountsWords.couldNotSave)) }
        ctx.interpolationQuality = .high
        ctx.draw(square, in: CGRect(x: 0, y: 0, width: px, height: px))
        guard let out = ctx.makeImage(), let png = NSBitmapImageRep(cgImage: out).representation(using: .png, properties: [:]) else {
            return .failure(.words(AccountsWords.couldNotSave))
        }
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            let url = folder.appendingPathComponent("\(accountId).png")
            try png.write(to: url, options: .atomic)
            return .success(url.path)
        } catch { return .failure(.words(AccountsWords.couldNotSave)) }
    }

    /// A stand-in picture for the driven fixture (`fixture-upload-ok` / `-over`): the size gate sees `declaredBytes`.
    static func fixtureUpload(declaredBytes: Int, accountId: String) -> Result<String, StoreError> {
        if let why = verdict(byteCount: declaredBytes) { return .failure(.words(why)) }
        let image = NSImage(size: NSSize(width: 96, height: 64), flipped: false) { r in
            NSColor.systemTeal.setFill(); r.fill(); return true
        }
        guard let tiff = image.tiffRepresentation, let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else {
            return .failure(.words(AccountsWords.couldNotSave))
        }
        return store(png, accountId: accountId)
    }

    enum StoreError: Error { case words(String)
        var words: String { switch self { case .words(let w): return w } }
    }
}
#endif
