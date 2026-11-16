import SwiftUI

/// A picker row: title left, detail right, the board's `.r` hover tint, the selection colour with white text on the
/// highlighted line. Both pickers share it, so they have the same height and radius.
struct PickerRow<Trailing: View>: View {
    let title: String
    var indent: Int = 0
    var highlighted = false
    var dimmed = false
    @ViewBuilder var trailing: () -> Trailing
    @State private var hovering = false

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text(title).whooshText(WhooshTypography.body, emphasized: highlighted)
                .foregroundStyle(highlighted ? WhooshPalette.onAccent : (dimmed ? WhooshPalette.secondaryLabel : WhooshPalette.label))
                .padding(.leading, CGFloat(indent) * WhooshSpacing.s16)
            Spacer(minLength: WhooshSpacing.s8)
            trailing()
        }
        .padding(.horizontal, WhooshSpacing.s10)
        .frame(height: WhooshMetrics.sidebarRow + WhooshSpacing.s4)
        .background(highlighted ? AnyShapeStyle(WhooshPalette.selection) : (hovering ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(.clear)),
                    in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .animation(WhooshMotion.hover, value: hovering)
    }
}

/// The panel both pickers sit in: `--panel` fill, sheet radius, hairline edge, the floating shadow.
struct PickerPanel<Content: View>: View {
    let width: CGFloat
    @ViewBuilder var content: () -> Content
    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s2) { content() }
            .padding(WhooshSpacing.s8)
            .frame(width: width)
            .background(WhooshPalette.panel, in: RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous))
            .overlay { RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
            .popoverEntrance()
    }
}

/// Snooze in words: later today, tomorrow morning, next week, or a date. A time that has passed is refused in a sentence.
public struct TimePickerInWords: View {
    let now: Date
    var onPick: (Date) -> Void
    @State private var highlighted: SnoozeOption.Kind? = .laterToday
    @State private var chosen = Date()
    @State private var showDate: Bool
    @State private var refusal: String?

    /// `start`: the date the calendar step opens on; nil keeps the gallery's sample (an hour ago, which shows the refusal).
    public init(now: Date, openOnDate: Bool = false, start: Date? = nil, onPick: @escaping (Date) -> Void) {
        self.now = now; self.onPick = onPick
        _showDate = State(initialValue: openOnDate)
        _chosen = State(initialValue: start ?? now.addingTimeInterval(-3600))
        _refusal = State(initialValue: nil)
    }

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    public var body: some View {
        // Two steps in one popover: the list turns into a calendar in place (cross-fade, the panel resizes on the system spring).
        PickerPanel(width: 280) {
            if showDate { calendarStep.transition(.opacity) } else { listStep.transition(.opacity) }
            if let refusal { reason(refusal).transition(.opacity) }
        }
        .animation(reduceMotion ? WhooshMotion.readerSwap : WhooshMotion.unfold, value: showDate)
        .animation(WhooshMotion.readerSwap, value: refusal)
    }

    private var listStep: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
            Text("Snooze until").whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                .padding(.horizontal, WhooshSpacing.s10).padding(.top, WhooshSpacing.s4)
            ForEach(TimeInWords.options(now: now)) { option in
                Button { choose(option) } label: {
                    PickerRow(title: option.title, highlighted: highlighted == option.kind && option.refusal == nil, dimmed: option.refusal != nil) {
                        if let when = option.when {
                            Text(TimeInWords.describe(when, now: now)).whooshText(WhooshTypography.caption)
                                .foregroundStyle(highlighted == option.kind && option.refusal == nil ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                        } else {
                            Image(systemName: "calendar").font(.system(size: WhooshMetrics.actionIcon - 3)).foregroundStyle(WhooshPalette.secondaryLabel)
                        }
                    }
                }
                .buttonStyle(.plain)
                .accessibilityLabel(option.when.map { "\(option.title), \(TimeInWords.describe($0, now: now))" } ?? option.title)
                .accessibilityHint(option.refusal ?? "")
            }
        }
    }

    private var calendarStep: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Button { showDate = false; refusal = nil } label: {
                HStack(spacing: WhooshSpacing.s4) {
                    Image(systemName: "chevron.left").font(.system(size: 10, weight: .semibold))
                    Text("Snooze until").whooshText(WhooshTypography.caption, emphasized: true)
                }.foregroundStyle(WhooshPalette.accent)
            }.buttonStyle(.plain).padding(.horizontal, WhooshSpacing.s10).padding(.top, WhooshSpacing.s4)
            .accessibilityLabel("Back to the snooze words")
            DatePicker("", selection: $chosen, displayedComponents: [.date]).labelsHidden().datePickerStyle(.graphical)
                .accessibilityLabel("Snooze until this date").padding(.horizontal, WhooshSpacing.s4)
                .frame(maxWidth: .infinity, alignment: .center)   // the system calendar is narrower than the panel: centred, not left with an empty half
            #if os(macOS)
            DatePicker("", selection: $chosen, displayedComponents: [.hourAndMinute]).labelsHidden().datePickerStyle(.field)
                .accessibilityLabel("Snooze until this time").padding(.horizontal, WhooshSpacing.s10)
            #else
            DatePicker("", selection: $chosen, displayedComponents: [.hourAndMinute]).labelsHidden().datePickerStyle(.compact)
                .accessibilityLabel("Snooze until this time").padding(.horizontal, WhooshSpacing.s10)
            #endif
            Button {
                switch TimeInWords.validate(chosen, now: now) {
                case .accepted(let d): refusal = nil; onPick(d)
                case .refused(let r): refusal = r
                }
            } label: {
                Text("Snooze").whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.onAccent)
                    .frame(maxWidth: .infinity).frame(height: WhooshMetrics.segmentHeight)
                    .background(WhooshPalette.selection, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            }.buttonStyle(.plain).padding(.horizontal, WhooshSpacing.s10)
        }
    }

    private func reason(_ text: String) -> some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s6) {
            Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot).padding(.top, WhooshSpacing.s6)
            Text(text).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
        }.padding(.horizontal, WhooshSpacing.s10).padding(.bottom, WhooshSpacing.s4)
    }

    private func choose(_ option: SnoozeOption) {
        highlighted = option.kind
        if let r = option.refusal { refusal = r; return }
        guard let when = option.when else { showDate = true; return }
        switch TimeInWords.validate(when, now: now) {
        case .accepted(let d): refusal = nil; onPick(d)
        case .refused(let r): refusal = r
        }
    }
}

/// The label picker: every account's labels nested, a search field, Create when the name is new.
public struct LabelPicker: View {
    @Binding var model: LabelPickerModel
    var onCreate: (String) -> Void
    @State private var highlight: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    public init(model: Binding<LabelPickerModel>, onCreate: @escaping (String) -> Void) { _model = model; self.onCreate = onCreate }

    public var body: some View {
        PickerPanel(width: 280) {
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: "magnifyingglass").font(.system(size: 11)).foregroundStyle(WhooshPalette.secondaryLabel)
                TextField("Label as…", text: $model.query).textFieldStyle(.plain).whooshText(WhooshTypography.body)
                    .accessibilityLabel("Search or name a label")
            }
            .padding(.horizontal, WhooshSpacing.s10).frame(height: WhooshMetrics.segmentHeight + WhooshSpacing.s2)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            .overlay { RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
            .padding(.bottom, WhooshSpacing.s4)
            ForEach(model.visibleRows()) { row in
                Button { model.toggle(row) } label: {
                    PickerRow(title: row.name, indent: model.query.isEmpty ? row.depth : 0,
                              highlighted: highlight == row.id) {
                        HStack(spacing: WhooshSpacing.s4) {
                            trailing(row)
                            if row.hasChildren, model.query.isEmpty { chevron(row) }
                        }
                    }
                }
                .buttonStyle(.plain)
                .onHover { if $0 { highlight = row.id } else if highlight == row.id { highlight = nil } }
                .accessibilityLabel(row.depth > 0 ? "\(row.name), in \(row.breadcrumb)" : row.name)
                .accessibilityAddTraits(model.selected.contains(row.id) ? .isSelected : [])
                // Folding a group reflows the rows below on the system spring; rows fade and settle from their parent (transform + opacity).
                .transition(reduceMotion ? .opacity : .opacity.combined(with: .scale(scale: ComponentMotion.popoverScaleFrom, anchor: .top)))
            }
            if let name = model.createOffer {
                Button { onCreate(name) } label: {
                    PickerRow(title: "Create “\(name)”", highlighted: highlight == "create") {
                        Image(systemName: "plus").font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(highlight == "create" ? WhooshPalette.onAccent : WhooshPalette.accent)
                    }
                }
                .buttonStyle(.plain)
                .onHover { highlight = $0 ? "create" : nil }
                .transition(.opacity)
            }
        }
        .animation(reduceMotion ? WhooshMotion.readerSwap : WhooshMotion.unfold, value: model.collapsed)
        .animation(WhooshMotion.readerSwap, value: model.createOffer)
    }

    private func chevron(_ row: LabelRow) -> some View {
        Button { model.toggleGroup(row) } label: {
            Image(systemName: "chevron.right").font(.system(size: 9, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
                .rotationEffect(.degrees(model.collapsed.contains(row.id) ? 0 : 90)).frame(width: WhooshSpacing.s16, height: WhooshSpacing.s16)
        }.buttonStyle(.plain).accessibilityLabel(model.collapsed.contains(row.id) ? "Expand \(row.name)" : "Collapse \(row.name)")
    }

    @ViewBuilder private func trailing(_ row: LabelRow) -> some View {
        let on = model.selected.contains(row.id)
        if on { Image(systemName: "checkmark").font(.system(size: 11, weight: .semibold)).foregroundStyle(highlight == row.id ? WhooshPalette.onAccent : WhooshPalette.accent) }
        else if !model.query.isEmpty, !row.breadcrumb.isEmpty { Text(row.breadcrumb).whooshText(WhooshTypography.caption).foregroundStyle(highlight == row.id ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel) }
    }
}
