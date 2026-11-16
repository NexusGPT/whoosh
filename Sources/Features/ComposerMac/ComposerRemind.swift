import SwiftUI

/// The panel behind the bell and ⌘⇧H: the shared time words (later today, tomorrow morning, next week, a date), under its own
/// title. A time that has passed is refused in a sentence. When a reminder is set, a row takes it away.
/// The date line (its picker, its button, a refusal) lives on the model so a click door reaches it (doors-compose).
struct RemindPanel: View {
    @Bindable var model: ComposerModel
    @Bindable var app: AppModel

    var body: some View {
        PickerPanel(width: WhooshSpacing.s32 * 9) {
            Text(ComposerWords.remindBy).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                .padding(.horizontal, WhooshSpacing.s10).padding(.top, WhooshSpacing.s4)
            ForEach(TimeInWords.options(now: Date()).filter { $0.kind != .pickDate }) { o in
                Button { pick(o) } label: {
                    PickerRow(title: o.title, highlighted: o.kind == .tomorrowMorning, dimmed: o.refusal != nil) {
                        Text(o.when.map { TimeInWords.describe($0, now: Date()) } ?? "")
                            .whooshText(WhooshTypography.caption)
                            .foregroundStyle(o.kind == .tomorrowMorning ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                    }
                }.buttonStyle(.plain).whooshAx(ComposerIds.remind + "." + o.kind.rawValue)
            }
            Button { withAnimation(WhooshMotion.unfold) { _ = model.toggleDatePicker() } } label: {
                PickerRow(title: ComposerWords.pickDate) { Image(systemName: "calendar").foregroundStyle(WhooshPalette.secondaryLabel) }
            }.buttonStyle(.plain).whooshAx(ComposerIds.remindPick)
            if model.datePicking {
                HStack {
                    DatePicker("", selection: $model.dateChosen, in: Date()..., displayedComponents: [.date, .hourAndMinute]).labelsHidden()
                    Button(ComposerWords.remindSetButton) { _ = model.setChosenReminder(app) }
                        .buttonStyle(.plain).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
                        .whooshAx(ComposerIds.remindSet)
                }.padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s4)
            }
            if model.reminderAt != nil {
                Button { _ = model.clearReminder() } label: { PickerRow(title: ComposerWords.remindClear) {} }
                    .buttonStyle(.plain).whooshAx(ComposerIds.remindClear)
            }
            if let refusal = model.dateRefusal {
                Text(refusal).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.warning).padding(.horizontal, WhooshSpacing.s10)
            }
        }
        .popoverEntrance(anchor: .bottomLeading)
    }

    private func pick(_ o: SnoozeOption) {
        guard let d = o.when, o.refusal == nil else { model.dateRefusal = o.refusal; return }
        _ = model.setReminder(d, app)
    }
}
