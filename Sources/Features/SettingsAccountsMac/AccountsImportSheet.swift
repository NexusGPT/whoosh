#if os(macOS)
import SwiftUI

/// Settings > Accounts > "Import mail…": the sources found on this Mac with their message count, "Choose a file…", the Full Disk Access ask,
/// the progress with Cancel, and the result. It draws the first start's own model (`ImportModel.shared`) in this pane's rows and buttons,
/// so the two never disagree; a second run of the same source stores nothing twice (the core's rule, shown as "Nothing new").
struct AccountsImportSheet: View {
    let session: AccountsSession
    @State private var model = ImportModel.shared
    private let P = "mac-settings-accounts.import"

    var body: some View {
        ZStack {
            Rectangle().fill(WhooshPalette.windowScrim).onTapGesture { if !model.running { session.closeImport() } }
            VStack(alignment: .leading, spacing: 0) {
                Text(ImportWords.title).font(.system(size: 15, weight: .semibold)).foregroundStyle(WhooshPalette.label)
                Text(ImportWords.subtitle).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel).padding(.top, 4)
                sources
                if model.run != nil { progress }
                if let words = model.refusalWords { refusal(words) }
                actions
            }
            .padding(20).frame(width: 420)
            .background(RoundedRectangle(cornerRadius: SettingsMetrics.cardRadius + 4, style: .continuous).fill(WhooshPalette.background))
            .overlay(RoundedRectangle(cornerRadius: SettingsMetrics.cardRadius + 4, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
            .shadow(color: .black.opacity(0.22), radius: 24, y: 10)
            .accessibilityElement(children: .contain).whooshAx("\(P).sheet")
        }
        // Escape closes the sheet unless a run is going (Cancel stops that): the same zero-size button as the removal review.
        .background { Button("") { if !model.running { session.closeImport() } }.keyboardShortcut(.cancelAction).opacity(0).frame(width: 0, height: 0).accessibilityHidden(true) }
        .transition(.opacity)
    }

    private var sources: some View {
        SettingsGroup(title: nil) {
            if model.sources.isEmpty {
                SettingsRow(ax: "\(P).empty", title: model.scanning ? ImportWords.scanning : ImportWords.nothingFound) { EmptyView() }
            }
            ForEach(model.sources) { s in
                SettingsRow(ax: "\(P).source.\(s.kind)", title: s.kind == "apple-mail" ? ImportWords.appleMail : s.name, caption: caption(s),
                            onTap: { if !model.running { model.select(s.id) } }) {
                    if s.readable { SettingsCheck(on: s.id == model.selected) } else { Circle().fill(WhooshPalette.warning).frame(width: 7, height: 7) }
                }
            }
            SettingsRow(ax: "\(P).choose.row", title: AccountsImportWords.chooseFile, caption: ImportWords.chooseHint) {
                SettingsButton(title: ImportWords.choose, ax: "\(P).choose") { if !model.running { ImportPicker.choose(model) } }
            }
        }
    }

    private func caption(_ s: ImportSourceItem) -> String {
        var parts = [s.count.map { ImportWords.count($0) } ?? (s.readable ? ImportWords.countUnknown : "")]
        if s.imported > 0 { parts.append(ImportWords.alreadyImported(s.imported)) }
        return ([ImportWords.kind(s.kind)] + parts.filter { !$0.isEmpty }).joined(separator: " · ")
    }

    private var progress: some View {
        VStack(alignment: .leading, spacing: 6) {
            ZStack(alignment: .leading) {
                Capsule().fill(WhooshPalette.fill)
                Capsule().fill(WhooshPalette.accent).scaleEffect(x: max(0.02, model.run?.fraction ?? (model.run?.isOver == true ? 1 : 0.02)), y: 1, anchor: .leading)
            }.frame(height: 4).clipShape(Capsule())
            Text(model.running ? model.progressLine : resultLine).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel)
                .lineLimit(1).truncationMode(.middle).whooshAx("\(P).progress")
        }
        .padding(.top, 14)
    }

    /// The finished run in one line; a run that stored nothing says so (a second pass over the same mail).
    var resultLine: String { AccountsImportLine.of(model) }

    private func refusal(_ words: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Image(systemName: "exclamationmark.circle.fill").foregroundStyle(WhooshPalette.warning)
                Text(model.needsFullDiskAccess ? AccountsImportWords.needsAccess : words).font(.system(size: 12)).foregroundStyle(WhooshPalette.label)
                    .fixedSize(horizontal: false, vertical: true).whooshAx("\(P).refusal")
            }
            if model.needsFullDiskAccess { SettingsButton(title: ImportWords.openFullDiskAccess, ax: "\(P).open-settings") { ImportPicker.openFullDiskAccess() } }
        }
        .padding(.top, 14)
    }

    private var actions: some View {
        HStack(spacing: 8) {
            Spacer()
            if model.running {
                SettingsButton(title: ImportWords.cancel, ax: "\(P).cancel") { model.cancel() }
            } else {
                SettingsButton(title: ImportWords.close, key: "esc", ax: "\(P).close") { session.closeImport() }
                if model.run?.state != "finished" {
                    SettingsButton(title: ImportWords.start, primary: true, ax: "\(P).start") { if model.canStart { model.start() } }
                }
            }
        }
        .padding(.top, 16)
    }
}

#endif

/// What the finished run says, for the sheet and for the check's row.
@MainActor enum AccountsImportLine {
    static func of(_ m: ImportModel) -> String {
        guard let r = m.run, r.isOver else { return "" }
        if r.state == "cancelled" { return ImportWords.cancelled }
        guard r.state == "finished", let x = m.result else { return "" }
        if x.stored == 0 && x.alreadyThere > 0 { return AccountsImportWords.nothingNew }
        let into = m.intoAccount.flatMap { id in m.accounts.first { $0.id == id }?.name } ?? Words.text("Imported")
        return AccountsImportWords.imported(x.stored, into: into)
    }
}
