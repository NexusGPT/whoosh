import SwiftUI

/// The provider whose mark sits in the glass chip. A known provider shows its own colours; an unknown domain keeps a grey globe.
/// The marks are the board's drawings (approximations: shipping needs the licensed marks, see docs/lanes/addaccount.md "Doors").
enum ProviderKind: String, CaseIterable {
    case google, microsoft, icloud, yahoo, fastmail, unknown

    var name: String {
        switch self {
        case .google: return "Google"; case .microsoft: return "Microsoft"; case .icloud: return "iCloud"
        case .yahoo: return "Yahoo"; case .fastmail: return "Fastmail"; case .unknown: return Words.text("the server")
        }
    }

    /// The provider the detection reports (contract `DetectionResult.provider`), as a mark kind.
    static func of(detected p: DetectedProvider) -> ProviderKind {
        switch p { case .microsoft365: return .microsoft; case .google: return .google; case .icloud: return .icloud; case .fastmail: return .fastmail; default: return .unknown }
    }

    /// The domain-to-provider table the detection cache reads while the person types: consumer domains and the fixture tenants.
    static func of(domain d: String) -> ProviderKind {
        let d = d.lowercased()
        func ends(_ s: String...) -> Bool { s.contains { d == $0 || d.hasSuffix("." + $0) } }
        if ends("gmail.com", "googlemail.com") { return .google }
        if ends("outlook.com", "hotmail.com", "live.com", "msn.com", "office365.com", "m365.example.test", "blocked-m365.example.test") { return .microsoft }
        if ends("icloud.com", "me.com", "mac.com") { return .icloud }
        if ends("yahoo.com", "ymail.com", "yahoo.co.uk", "yahoo.fr") { return .yahoo }
        if ends("fastmail.com", "fastmail.fm", "messagingengine.com") { return .fastmail }
        return .unknown
    }

    /// The provider an address names, or nil while it is not an address yet (no globe before a domain exists).
    static func of(address a: String) -> ProviderKind? {
        guard AddAccountModel.looksLikeAddress(a), let d = a.split(separator: "@").last else { return nil }
        return of(domain: String(d))
    }

    /// The provider a detected server names (a custom domain on Google Workspace reads imap.gmail.com).
    static func of(host h: String) -> ProviderKind {
        let h = h.lowercased()
        if h.contains("gmail") || h.contains("google") { return .google }
        if h.contains("office365") || h.contains("outlook") || h.contains("hotmail") { return .microsoft }
        if h.contains("me.com") || h.contains("icloud") { return .icloud }
        if h.contains("yahoo") { return .yahoo }
        if h.contains("fastmail") || h.contains("messagingengine") { return .fastmail }
        return .unknown
    }

    /// The mark's size as a share of the chip: the cloud is wide, the four squares are heavy (board `PM[k].s`).
    var share: CGFloat {
        switch self { case .google: return 0.56; case .microsoft: return 0.50; case .icloud: return 0.64; case .yahoo: return 0.58; case .fastmail: return 0.66; case .unknown: return 0.62 }
    }
}

/// The mark itself, drawn in a 48-unit box (Microsoft in 24), light or dark.
struct ProviderMarkShape: View {
    let kind: ProviderKind
    @Environment(\.colorScheme) private var scheme
    private var dark: Bool { scheme == .dark }

    var body: some View {
        switch kind {
        case .google:
            ZStack {
                SVGShape(d: "M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z", box: 48).fill(Color(hex: 0xEA4335))  // brand-mark: google
                SVGShape(d: "M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z", box: 48).fill(Color(hex: 0x4285F4))  // brand-mark: google
                SVGShape(d: "M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z", box: 48).fill(Color(hex: 0xFBBC05))  // brand-mark: google
                SVGShape(d: "M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z", box: 48).fill(Color(hex: 0x34A853))  // brand-mark: google
            }
        case .microsoft:
            GeometryReader { g in
                let u = g.size.width / 24
                ForEach(Array([(1.5, 1.5, 0xF25022), (12.5, 1.5, 0x7FBA00), (1.5, 12.5, 0x00A4EF), (12.5, 12.5, 0xFFB900)].enumerated()), id: \.offset) { _, q in  // brand-mark: microsoft
                    Rectangle().fill(Color(hex: UInt32(q.2))).frame(width: 10 * u, height: 10 * u).offset(x: q.0 * u, y: q.1 * u)  // brand-mark: microsoft
                }
            }
        case .icloud:
            SVGShape(d: "M13.5 37.5h22.2a8.3 8.3 0 0 0 1.3-16.5A11.6 11.6 0 0 0 15 18.2a9.7 9.7 0 0 0-1.5 19.3z", box: 48)
                .fill(LinearGradient(colors: [Color(hex: dark ? 0x7CC6FF : 0x5DB8FF), Color(hex: dark ? 0x3B8DF0 : 0x1A73E8)], startPoint: .top, endPoint: .bottom))  // brand-mark: icloud
        case .yahoo:
            let purple = Color(hex: dark ? 0xA06BFF : 0x6001D2)  // brand-mark: yahoo
            GeometryReader { g in
                let u = g.size.width / 48
                SVGShape(d: "M7 11l10.5 15.5L28 11M17.5 26.5V38M38.5 10.5v16", box: 48).stroke(purple, style: StrokeStyle(lineWidth: 5.6 * u, lineCap: .round, lineJoin: .round))
                Circle().fill(purple).frame(width: 6.6 * u, height: 6.6 * u).offset(x: (38.5 - 3.3) * u, y: (36.2 - 3.3) * u)
            }
        case .fastmail:
            let grad = LinearGradient(colors: [Color(hex: dark ? 0x6BAEFF : 0x2B7DE1), Color(hex: dark ? 0x3D86F0 : 0x0B4EA2)], startPoint: .top, endPoint: .bottom)  // brand-mark: fastmail
            GeometryReader { g in
                let u = g.size.width / 48
                let st = StrokeStyle(lineWidth: 4 * u, lineCap: .round, lineJoin: .round)
                RoundedRectangle(cornerRadius: 3.5 * u).path(in: CGRect(x: 12 * u, y: 12.5 * u, width: 28 * u, height: 22 * u)).stroke(grad, style: st)
                SVGShape(d: "M13.5 15L26 24.5 38.5 15", box: 48).stroke(grad, style: st)
                SVGShape(d: "M3.5 29.5h5", box: 48).stroke(Color(hex: dark ? 0xFFB547 : 0xF5A623), style: StrokeStyle(lineWidth: 3.6 * u, lineCap: .round))  // brand-mark: fastmail
            }
        case .unknown:
            Image(systemName: "globe").resizable().scaledToFit().foregroundStyle(WhooshPalette.secondaryLabel)
        }
    }
}

extension Color {
    init(hex: UInt32) { self.init(red: Double((hex >> 16) & 255) / 255, green: Double((hex >> 8) & 255) / 255, blue: Double(hex & 255) / 255) }
}
