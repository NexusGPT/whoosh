import SwiftUI

/// One account row: mark, name and address, then what is happening to it in words, then a thin progress bar.
/// A tap opens the editor in place (name, colour, icon, signature). A waiting row asks for its password inline.
struct WelcomeRowView: View {
    let row: WelcomeRow
    let model: WelcomeModel
    @State private var secret = ""
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var open: Bool { model.editing == row.id }
    private var route: WelcomeRoute { WelcomeRoutes.detect(row.address) }

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            HStack(spacing: WhooshSpacing.s10) {
                AccountMark(name: row.name, address: row.address, colour: row.colour,
                            icon: row.iconId.map { .library($0) } ?? .providerDefault, size: .compact, needsAttention: row.phase == .refused)
                VStack(alignment: .leading, spacing: 0) {
                    Text(row.name).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(1)
                    Text(row.address).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                }
                Spacer(minLength: WhooshSpacing.s8)
                statusView
            }
            if row.phase == .waiting { passwordLine }
            if row.phase == .refused { refusedLine }
            if row.inFlight { progress }
            if open { editor.transition(.opacity) }
        }
        .padding(.vertical, WhooshSpacing.s6)
        .contentShape(Rectangle())
        .onTapGesture { withAnimation(WhooshMotion.animation(WhooshMotion.unfold, reduceMotion: reduceMotion)) { model.editing = open ? nil : row.id } }
        .whooshAx("mac-welcome-accounts.accounts.\(row.id)")
    }

    private var statusView: some View {
        Group {
            switch row.phase {
            case .healthy: Label(Words.text("Ready"), systemImage: "checkmark.circle.fill").foregroundStyle(WhooshPalette.secondaryLabel)
            case .refused: Label(Words.text("Not connected"), systemImage: "exclamationmark.circle.fill").foregroundStyle(WhooshPalette.warning)
            default: Text(row.progressWords).foregroundStyle(WhooshPalette.secondaryLabel)
            }
        }
        .labelStyle(.titleAndIcon).whooshText(WhooshTypography.caption, emphasized: true)
    }

    private var passwordLine: some View {
        HStack(spacing: WhooshSpacing.s8) {
            if route.usesSheet {
                Text(route.how).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                Spacer()
                Button(Words.text("Sign in with %@", route.name)) { model.submitSecret(row.id, "") }.buttonStyle(WelcomeFilledStyle())
                    .accessibilityIdentifier("mac-welcome-accounts.sign-in.\(row.id)")
            } else {
                SecureField(route.kind == .proton ? Words.text("Bridge password") : Words.text("App password"), text: $secret)
                    .textFieldStyle(.plain).whooshText(WhooshTypography.subhead).padding(.horizontal, WhooshSpacing.s10).frame(height: 26)
                    .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).strokeBorder(WhooshPalette.separator))
                    .onSubmit { send() }
                Button(Words.text("Connect")) { send() }.buttonStyle(WelcomeFilledStyle()).disabled(secret.isEmpty)
            }
        }.padding(.leading, WhooshSpacing.s32 - WhooshSpacing.s4)
    }

    private func send() { guard !secret.isEmpty else { return }; model.submitSecret(row.id, secret); secret = "" }

    private var refusedLine: some View {
        HStack(spacing: WhooshSpacing.s6) {
            Text(Words.text("Refused: %@", row.status)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.label)
                .accessibilityIdentifier("mac-welcome-accounts.accounts.\(row.id).why")
            if let u = URL(string: row.statusLink), !row.statusLink.isEmpty { Link(route.linkWords ?? Words.text("Open the page"), destination: u).font(.system(size: 11, weight: .semibold)) }
            Spacer()
            Button(Words.text("Try again")) { model.tryAgain(row.id) }.buttonStyle(.plain)
                .whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.accent)
        }.padding(.leading, WhooshSpacing.s32 - WhooshSpacing.s4)
    }

    private var progress: some View {
        Capsule().fill(WhooshPalette.fill).frame(height: WhooshMetrics.progressBar)
            .overlay(alignment: .leading) {
                GeometryReader { g in
                    Capsule().fill(WhooshPalette.accent).frame(width: g.size.width * (row.phase == .connecting ? 0.35 : 0.8))
                        .animation(WhooshMotion.animation(WhooshMotion.newRow, reduceMotion: reduceMotion), value: row.phase)
                }
            }.padding(.leading, WhooshSpacing.s32 - WhooshSpacing.s4)
    }

    private var editor: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s10) {
            field(Words.text("Name"), text: Binding(get: { row.name }, set: { v in model.set(row.id) { $0.name = v } }))
            HStack(spacing: WhooshSpacing.s6) {
                ForEach(WelcomeColour.swatches, id: \.self) { hex in
                    Button { model.set(row.id) { $0.colour = hex } } label: {
                        AccountMark(name: row.name, address: row.address, colour: hex, icon: .providerDefault, size: .sidebar, chosen: hex == row.colour)
                    }.buttonStyle(.plain)
                }
            }
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(WhooshMetrics.actionButton), spacing: WhooshSpacing.s4), count: 10), alignment: .leading, spacing: WhooshSpacing.s4) {
                ForEach(AccountMarkIcon.libraryIds.keys.sorted(), id: \.self) { id in
                    Button { model.set(row.id) { $0.iconId = id } } label: {
                        Image(systemName: AccountMarkIcon.libraryIds[id] ?? "envelope").font(.system(size: 12))
                            .frame(width: WhooshMetrics.actionButton, height: WhooshMetrics.actionButton)
                            .background(row.iconId == id ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(Color.clear), in: RoundedRectangle(cornerRadius: WhooshRadius.libraryTile, style: .continuous))
                    }.buttonStyle(.plain).foregroundStyle(WhooshPalette.label).help(id)
                }
            }
            field(Words.text("Signature"), text: Binding(get: { row.signature }, set: { v in model.set(row.id) { $0.signature = v } }))
            Button(Words.text("Remove this address")) { model.remove(row.id) }.buttonStyle(.plain)
                .whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.warning)
        }
        .padding(.leading, WhooshSpacing.s32 - WhooshSpacing.s4).padding(.top, WhooshSpacing.s4)
    }

    private func field(_ title: String, text: Binding<String>) -> some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text(title).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).frame(width: 64, alignment: .leading)
            TextField(title, text: text).textFieldStyle(.plain).whooshText(WhooshTypography.subhead).padding(.horizontal, WhooshSpacing.s10).frame(height: 26)
                .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).strokeBorder(WhooshPalette.separator))
        }
    }
}

/// Filled control: white text on the selection blue (WhooshPalette.filledControl), a press darkens the same frame.
struct WelcomeFilledStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.onAccent)
            .padding(.horizontal, WhooshSpacing.s12).frame(height: WhooshMetrics.segmentHeight - 4)
            .background(WhooshPalette.filledControl, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            .opacity(configuration.isPressed ? 0.82 : 1)
    }
}
