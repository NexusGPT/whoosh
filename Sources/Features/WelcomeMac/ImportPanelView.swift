import SwiftUI
#if os(macOS)
import AppKit
#endif

/// Motion of the import panel: opacity and transform only, every duration one of the shared constants (docs/MOTION.md law 4: 260 ms at most).
enum ImportMotion {
    static func fade(_ reduce: Bool) -> Animation? { WhooshMotion.animation(.easeOut(duration: Double(WhooshConstants.POPOVER_MS) / 1000), reduceMotion: reduce) }
    static func bar(_ reduce: Bool) -> Animation? { WhooshMotion.animation(WhooshMotion.newRow, reduceMotion: reduce) }
    /// How the panel enters and leaves: it fades and rises by UNDO_RISE_PX; it never resizes in place.
    static var enter: AnyTransition { .opacity.combined(with: .offset(y: WhooshMotion.undoRise)) }
}

/// The optional import step of the first start, and the same panel behind Settings > Accounts "Import mail…":
/// the sources found on this Mac, a Choose… file picker, where the mail goes, Start, the progress line, Cancel, the Full Disk Access
/// refusal in the contract's words, and a way out (Skip to the inbox, or Close from Settings).
struct ImportPanelView: View {
    @State private var model = ImportModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
            header
            sourceList
            chooseRow
            if model.accounts.count > 0 { intoRow }
            if model.run != nil { runBlock.transition(.opacity) }
            if let words = model.refusalWords { refusalBlock(words).transition(.opacity) }
            actions
        }
        .padding(WhooshSpacing.s14).frame(maxWidth: .infinity, alignment: .leading)
        .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous).strokeBorder(WhooshPalette.separator))
        .animation(ImportMotion.fade(reduceMotion), value: model.run == nil)
        .animation(ImportMotion.fade(reduceMotion), value: model.refusalWords)
        .animation(ImportMotion.fade(reduceMotion), value: model.sources.map(\.id))
        .task { model.reconnect(); if !model.scanned { model.scan() } }
        #if os(macOS)
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            if model.needsFullDiskAccess && !model.running { model.scan() }   // the grant is made in System Settings: look again on return
        }
        #endif
        .accessibilityIdentifier("mac-welcome-accounts.import")
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
            Text(ImportWords.title).whooshText(WhooshTypography.title3, emphasized: true).foregroundStyle(WhooshPalette.label)
            Text(ImportWords.subtitle).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
        }
    }

    private var sourceList: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
            if model.sources.isEmpty {
                Text(model.scanning ? ImportWords.scanning : ImportWords.nothingFound).whooshText(WhooshTypography.subhead)
                    .foregroundStyle(WhooshPalette.secondaryLabel).accessibilityIdentifier("mac-welcome-accounts.import.empty")
            }
            ForEach(model.sources) { s in
                ImportSourceRowView(source: s, chosen: s.id == model.selected, locked: model.running) { model.select(s.id) }
            }
        }
    }

    private var chooseRow: some View {
        HStack(spacing: WhooshSpacing.s8) {
            #if os(macOS)
            Button(ImportWords.choose) { ImportPicker.choose(model) }.buttonStyle(.plain).whooshText(WhooshTypography.subhead, emphasized: true)
                .foregroundStyle(WhooshPalette.accent).disabled(model.running).accessibilityIdentifier("mac-welcome-accounts.import.choose")
            #endif
            Text(ImportWords.chooseHint).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel)
        }
    }

    private var intoRow: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text(ImportWords.into).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            Picker(ImportWords.into, selection: $model.intoAccount) {
                Text(ImportWords.matching).tag(String?.none)
                ForEach(model.accounts) { Text($0.name).tag(String?.some($0.id)) }
            }.labelsHidden().disabled(model.running).fixedSize().accessibilityIdentifier("mac-welcome-accounts.import.into")
        }
    }

    private var runBlock: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            ZStack(alignment: .leading) {
                Capsule().fill(WhooshPalette.fill)
                Capsule().fill(WhooshPalette.accent).scaleEffect(x: max(0.02, model.run?.fraction ?? (model.run?.isOver == true ? 1 : 0.02)), y: 1, anchor: .leading)
                    .animation(ImportMotion.bar(reduceMotion), value: model.run?.fraction)
            }.frame(height: 4).clipShape(Capsule())
            Text(model.running ? model.progressLine : model.finishLine).whooshText(WhooshTypography.caption)
                .foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1).truncationMode(.middle)
                .accessibilityIdentifier("mac-welcome-accounts.import.progress")
        }
    }

    private func refusalBlock(_ words: String) -> some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(words).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label).fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("mac-welcome-accounts.import.refusal")
            #if os(macOS)
            if model.needsFullDiskAccess {
                Button(ImportWords.openFullDiskAccess) { ImportPicker.openFullDiskAccess() }.buttonStyle(WelcomeFilledStyle())
                    .accessibilityIdentifier("mac-welcome-accounts.import.open-settings")
            }
            #endif
        }
        .padding(WhooshSpacing.s10).frame(maxWidth: .infinity, alignment: .leading)
        .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
        .overlay(alignment: .topTrailing) { Circle().fill(WhooshPalette.warning).frame(width: 7, height: 7).padding(WhooshSpacing.s8) }
    }

    private var actions: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Button(model.fromSettings ? ImportWords.close : ImportWords.skip) { _ = model.leave(AppModel.shared) }
                .buttonStyle(.plain).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                .accessibilityIdentifier("mac-welcome-accounts.import.skip")
            Spacer(minLength: WhooshSpacing.s8)
            if model.running {
                Button(ImportWords.cancel) { model.cancel() }.buttonStyle(.plain).whooshText(WhooshTypography.subhead, emphasized: true)
                    .foregroundStyle(WhooshPalette.accent).accessibilityIdentifier("mac-welcome-accounts.import.cancel")
            } else if model.run?.isOver == true {
                Button(ImportWords.goInbox) { _ = model.leave(AppModel.shared) }.buttonStyle(WelcomeFilledStyle())
                    .accessibilityIdentifier("mac-welcome-accounts.import.done")
            } else {
                Button(ImportWords.start) { model.start() }.buttonStyle(WelcomeFilledStyle()).disabled(!model.canStart)
                    .accessibilityIdentifier("mac-welcome-accounts.import.start")
            }
        }
    }
}
