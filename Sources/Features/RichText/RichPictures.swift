import Foundation
import ImageIO
#if canImport(AppKit)
import AppKit
#else
import UIKit
#endif

/// A picture run in the text (`RichRun.cid`) is drawn as the picture, not as a symbol. The composer registers the file a content id names
/// (`register(cid:url:)`); `RichAttributed.make` asks `cached(_:)` and, when the thumbnail is not made yet, draws the placeholder and calls
/// `load(_:)`, which decodes a thumbnail of at most `maxSide` pixels on a utility queue (ImageIO, never the full picture, never the main
/// thread) and posts `didLoad` on the main queue with the content id. At most `cacheLimit` thumbnails are kept, so a long draft stays light.
final class RichPictures: @unchecked Sendable {
    static let shared = RichPictures()
    static let maxSide = WhooshConstants.RICH_PICTURE_THUMB_PX   // Int; the ledger's rows
    static let cacheLimit = WhooshConstants.RICH_PICTURE_CACHE_MAX
    /// The longest side the picture is drawn at in the text, in points.
    static let drawnMaxSide: CGFloat = 280
    static let didLoad = Notification.Name("whoosh.rich.picture.loaded")

    private let lock = NSLock()
    private var files: [String: URL] = [:]
    private var loading: Set<String> = []
    private let cache = NSCache<NSString, PImage>()
    private let queue = DispatchQueue(label: "whoosh.rich.pictures", qos: .utility)

    init() { cache.countLimit = Self.cacheLimit }

    func register(cid: String, url: URL) {
        lock.lock(); files[cid] = url; lock.unlock()
    }

    func file(for cid: String) -> URL? {
        lock.lock(); defer { lock.unlock() }
        return files[cid]
    }

    func cached(_ cid: String) -> PImage? { cache.object(forKey: cid as NSString) }

    /// Starts the decode once per content id; a content id with no registered file does nothing (the placeholder stays).
    func load(_ cid: String) {
        lock.lock()
        guard cache.object(forKey: cid as NSString) == nil, !loading.contains(cid), let url = files[cid] else { lock.unlock(); return }
        loading.insert(cid)
        lock.unlock()
        queue.async { [self] in
            let image = Self.thumbnail(of: url, maxSide: Self.maxSide)
            if let image { cache.setObject(image, forKey: cid as NSString) }
            lock.lock(); loading.remove(cid); lock.unlock()
            if image != nil { DispatchQueue.main.async { NotificationCenter.default.post(name: Self.didLoad, object: nil, userInfo: ["cid": cid]) } }
        }
    }

    /// A thumbnail whose longer side is at most `maxSide` pixels; nil when the file is no picture. Decodes only the thumbnail.
    static func thumbnail(of url: URL, maxSide: Int) -> PImage? {
        guard let src = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
        let options: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true,
                                        kCGImageSourceThumbnailMaxPixelSize: maxSide]
        guard let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, options as CFDictionary) else { return nil }
        #if canImport(AppKit)
        return NSImage(cgImage: cg, size: NSSize(width: cg.width, height: cg.height))
        #else
        return UIImage(cgImage: cg)
        #endif
    }

    /// The size the picture is drawn at: its own, shrunk to fit `drawnMaxSide` on the longer side, never enlarged.
    static func drawnSize(width: CGFloat, height: CGFloat) -> CGSize {
        let longer = max(width, height)
        guard longer > drawnMaxSide, longer > 0 else { return CGSize(width: width, height: height) }
        let k = drawnMaxSide / longer
        return CGSize(width: (width * k).rounded(), height: (height * k).rounded())
    }
}
