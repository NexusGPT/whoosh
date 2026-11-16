import SwiftUI

/// The smart-folder rule editor: the rule in words, where the folder lives, and the live match count that ticks in
/// place (fixed-width digits, so nothing moves) while the owner types. Enter saves, Esc leaves it as it was.
struct SmartFolderEditor: View {
    @Bindable var model: LabelsContactsModel

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s20) {
            VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
                Text(LabelsWords.smartFolder).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                if model.isNewFolder {
                    TextField(LabelsWords.folderNameHint, text: $model.draftName)
                        .textFieldStyle(.plain).whooshText(WhooshTypography.largeTitle, emphasized: true)
                        .foregroundStyle(WhooshPalette.label)
                        .accessibilityIdentifier("mac-labels-contacts.folder.name")
                } else {
                    Text(model.editing?.name ?? "").whooshText(WhooshTypography.largeTitle, emphasized: true)
                        .foregroundStyle(WhooshPalette.label)
                }
            }
            field(LabelsWords.showsMail) { ruleField }
            field(LabelsWords.livesUnder) { parents }
            matchesNote
            if let words = model.folderRefusal {
                Label(words, systemImage: "exclamationmark.circle")
                    .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.warning)
                    .transition(.opacity)
                    .accessibilityIdentifier("mac-labels-contacts.rule.refusal")
            }
            Spacer(minLength: 0)
            HStack(spacing: WhooshSpacing.s16) {
                KeyHint("↩", LabelsWords.save); KeyHint("esc", LabelsWords.cancel)
                Spacer()
                Button(LabelsWords.cancel) { _ = model.cancel() }.keyboardShortcut(.cancelAction)
                    .accessibilityIdentifier("mac-labels-contacts.rule.cancel")
                Button(LabelsWords.save) { _ = model.save() }.keyboardShortcut(.defaultAction)
                    .accessibilityIdentifier("mac-labels-contacts.rule.save")
            }
        }
        .animation(WhooshMotion.hover, value: model.folderRefusal)
        .onChange(of: model.app.typed) { _, _ in if model.app.surface == LabelsContactsMacFeature.slot.id { model.takeTyped(model.app) } }   // the window stays alive after Esc or Save with the pane on .folder: only the labels surface owns the buffer
        .padding(.horizontal, WhooshMetrics.readerSide).padding(.top, WhooshMetrics.readerTop).padding(.bottom, WhooshMetrics.readerSide)
        .frame(maxWidth: WhooshMetrics.readerMeasure + 2 * WhooshMetrics.readerSide, maxHeight: .infinity, alignment: .topLeading)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func field<C: View>(_ title: String, @ViewBuilder _ content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(title).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            content()
        }
    }

    private var ruleField: some View {
        HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s12) {
            TextField(LabelsWords.ruleHint, text: $model.draftRule, axis: .vertical)
                .textFieldStyle(.plain).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                .lineLimit(1...3)
                .accessibilityIdentifier("mac-labels-contacts.rule.field")
            MatchCount(count: model.draftMatches)
        }
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s10)
        .background(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(WhooshPalette.field))
        .overlay(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).strokeBorder(WhooshPalette.separator))
    }

    private var parents: some View {
        HStack(spacing: WhooshSpacing.s6) {
            chip(nil, LabelsWords.topLevel)
            ForEach(model.labels.children(of: nil).map(\.name), id: \.self) { chip($0, $0) }
        }
    }

    private func chip(_ value: String?, _ title: String) -> some View {
        let on = model.draftParent == value
        return Button { _ = model.setParent(value) } label: {
            Text(title).whooshText(WhooshTypography.subhead, emphasized: on)
                .foregroundStyle(on ? WhooshPalette.onAccent : WhooshPalette.label)
                .padding(.horizontal, WhooshSpacing.s12).frame(height: WhooshMetrics.segmentHeight)
                .background(Capsule().fill(on ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.fill)))
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .animation(WhooshMotion.hover, value: on)
        .accessibilityIdentifier("mac-labels-contacts.parent.\(value.map(LabelsContactsSource.slug) ?? "none")")
    }

    /// The count itself ticks beside the field (from the core's `ruleMatchCount`); a rule that matches nothing says so in words.
    @ViewBuilder private var matchesNote: some View {
        if model.draftMatches == 0 {
            Text(LabelsWords.nothingMatches)
                .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                .transition(.opacity)
        }
    }
}

/// "3 matches": the digits roll in place, the word keeps its width; feedback lands on the keystroke's frame.
struct MatchCount: View {
    let count: Int
    var body: some View {
        HStack(spacing: WhooshSpacing.s4) {
            Text("\(count)").contentTransition(.numericText(value: Double(count)))
            Text(count == 1 ? "match" : "matches")
        }
        .whooshText(WhooshTypography.subhead, emphasized: true)
        .foregroundStyle(count == 0 ? WhooshPalette.secondaryLabel : WhooshPalette.accent)
        .fixedSize()
        .animation(.snappy, value: count)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("mac-labels-contacts.rule.matches")
    }
}
