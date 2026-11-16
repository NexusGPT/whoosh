import SwiftUI

/// Send later, in words (J3.8): the shared time rules (later today, tomorrow morning, next week, a date), under the
/// composer's own title. A time that has passed is refused in a sentence. The panel opens from its button.
/// The date line (its picker, its button, a refusal) lives on the model so a click door reaches it (doors-compose).
struct SendLaterPanel: View {
    @Bindable var model: ComposerModel
    @Bindable var app: AppModel

    var body: some View {
        PickerPanel(width: WhooshSpacing.s32 * 9) {
            Text(ComposerWords.sendAt).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                .padding(.horizontal, WhooshSpacing.s10).padding(.top, WhooshSpacing.s4)
            ForEach(TimeInWords.options(now: Date()).filter { $0.kind != .pickDate }) { o in
                Button { _ = model.scheduleOption(o.kind, app) } label: {
                    PickerRow(title: o.title, highlighted: o.kind == .tomorrowMorning, dimmed: o.refusal != nil) {
                        Text(o.when.map { TimeInWords.describe($0, now: Date()) } ?? "")
                            .whooshText(WhooshTypography.caption)
                            .foregroundStyle(o.kind == .tomorrowMorning ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                    }
                }.buttonStyle(.plain).whooshAx(ComposerIds.sendLater + "." + o.kind.rawValue)
            }
            Button { withAnimation(WhooshMotion.unfold) { _ = model.toggleDatePicker() } } label: {
                PickerRow(title: ComposerWords.pickDate) { Image(systemName: "calendar").foregroundStyle(WhooshPalette.secondaryLabel) }
            }.buttonStyle(.plain).whooshAx(ComposerIds.sendLaterPick)
            if model.datePicking {
                HStack {
                    DatePicker("", selection: $model.dateChosen, in: Date()..., displayedComponents: [.date, .hourAndMinute]).labelsHidden()
                    Button(ComposerWords.schedule) { _ = model.scheduleChosen(app) }
                        .buttonStyle(.plain).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
                        .whooshAx(ComposerIds.sendLaterSchedule)
                }.padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s4)
            }
            if let refusal = model.dateRefusal {
                Text(refusal).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.warning).padding(.horizontal, WhooshSpacing.s10)
            }
        }
        .popoverEntrance(anchor: .bottomTrailing)
    }
}
