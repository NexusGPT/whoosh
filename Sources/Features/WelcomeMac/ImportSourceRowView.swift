import SwiftUI

/// One source found on this Mac: a radio mark, its name, what it is and how many messages it holds, and a warning dot when it cannot be read yet.
struct ImportSourceRowView: View {
    let source: ImportSourceItem
    let chosen: Bool
    let locked: Bool
    let pick: () -> Void
    @State private var hover = false

    private var detail: String {
        var parts = [source.count.map { ImportWords.count($0) } ?? (source.readable ? ImportWords.countUnknown : "")]
        if source.imported > 0 { parts.append(ImportWords.alreadyImported(source.imported)) }
        return parts.filter { !$0.isEmpty }.joined(separator: " · ")
    }

    var body: some View {
        Button(action: pick) {
            HStack(spacing: WhooshSpacing.s10) {
                Image(systemName: chosen ? "largecircle.fill.circle" : "circle")
                    .font(.system(size: 14)).foregroundStyle(chosen ? WhooshPalette.accent : WhooshPalette.tertiaryLabel)
                VStack(alignment: .leading, spacing: 0) {
                    Text(source.kind == "apple-mail" ? ImportWords.appleMail : source.name).whooshText(WhooshTypography.body, emphasized: true)
                        .foregroundStyle(WhooshPalette.label).lineLimit(1)
                    Text(ImportWords.kind(source.kind) + (detail.isEmpty ? "" : " · " + detail)).whooshText(WhooshTypography.caption)
                        .foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                }
                Spacer(minLength: WhooshSpacing.s8)
                if !source.readable { Circle().fill(WhooshPalette.warning).frame(width: 7, height: 7) }
            }
            .padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s6)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).fill(WhooshPalette.fill).opacity(hover || chosen ? 1 : 0))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain).disabled(locked)
        .onHover { hover = $0 }
        .animation(WhooshMotion.hover, value: hover)
        .accessibilityIdentifier("mac-welcome-accounts.import.source.\(source.kind)")
    }
}
