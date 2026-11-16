import Foundation
import Observation
import SwiftUI
import CoreImage.CIFilterBuiltins
#if canImport(AppKit)
import AppKit
#endif

/// "Move accounts to iPhone": one pairing token behind a QR code, replaced by a new one when it expires.
/// The code carries the token only: no address, no password (DeviceSync/PairingToken).
@MainActor @Observable
public final class WelcomeQR {
    public static let shared = WelcomeQR()
    public private(set) var token: PairingToken
    public private(set) var issued = 1
    @ObservationIgnored private let service: PairingService
    @ObservationIgnored private let nowMs: () -> Int64

    public init(expirySeconds: Int = WhooshConstants.QR_EXPIRY_S, nowMs: @escaping () -> Int64 = WelcomeQR.socketClock()) {
        self.nowMs = nowMs
        service = PairingService(expirySeconds: expirySeconds, clock: nowMs)
        token = service.issue()
    }

    /// The socket clock: under the fixture core `clock advance` moves it, so a code can be expired on demand.
    @MainActor public static func socketClock() -> () -> Int64 {
        let c = AppModel.shared.host.clock
        return { c.nowMs() }
    }

    public func secondsLeft() -> Int { max(0, Int((token.expiresAtMs - nowMs() + 999) / 1000)) }
    public var expired: Bool { nowMs() >= token.expiresAtMs }

    /// A fresh code once the old one has expired. Returns true when it issued one.
    @discardableResult
    public func refreshIfExpired() -> Bool {
        guard expired else { return false }
        token = service.issue(); issued += 1
        return true
    }

    public static func clock(_ s: Int) -> String { String(format: "%d:%02d", s / 60, s % 60) }

    #if canImport(AppKit)
    public func image(side: CGFloat) -> NSImage? {
        let f = CIFilter.qrCodeGenerator()
        f.message = Data(token.qrPayload.utf8); f.correctionLevel = "M"
        guard let out = f.outputImage else { return nil }
        let scale = ceil(side * 2 / out.extent.width)
        let big = out.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
        let rep = NSCIImageRep(ciImage: big)
        let img = NSImage(size: NSSize(width: side, height: side)); img.addRepresentation(rep)
        return img
    }
    #endif
}

struct WelcomeQRCard: View {
    let qr = WelcomeQR.shared
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { _ in
            let _ = qr.refreshIfExpired()
            VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
                Text(Words.text("Move accounts to iPhone")).whooshText(WhooshTypography.title3, emphasized: true).foregroundStyle(WhooshPalette.label)
                HStack(alignment: .top, spacing: WhooshSpacing.s16) {
                    code
                    VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
                        Text(Words.text("Open Whoosh on your iPhone and scan this code.")).whooshText(WhooshTypography.subhead)
                            .foregroundStyle(WhooshPalette.label)
                        Text(Words.text("It moves your accounts, names, colours, order and signatures. Passwords travel through iCloud Keychain, never in this code."))
                            .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                        Text(Words.text("New code in %@. A fresh one appears by itself.", WelcomeQR.clock(qr.secondsLeft())))
                            .whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                            .accessibilityIdentifier("mac-welcome-accounts.qr.expiry")
                    }
                }
            }
            .padding(WhooshSpacing.s16)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
            .accessibilityIdentifier("mac-welcome-accounts.qr")
        }
    }

    @ViewBuilder private var code: some View {
        #if canImport(AppKit)
        if let img = qr.image(side: 132) {
            Image(nsImage: img).interpolation(.none).resizable().frame(width: 132, height: 132)
                .padding(WhooshSpacing.s8).background(Color.white, in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
        }
        #endif
    }
}
