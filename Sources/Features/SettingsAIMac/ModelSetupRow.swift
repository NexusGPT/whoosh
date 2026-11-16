import SwiftUI

/// What a model row of Settings shows (board ai-setup-r1, W1): ready, missing, downloading, or failed.
enum ModelPhase: Equatable {
    case ready
    case missing(line: String)
    case downloading(line: String, fraction: Double?)
    case failed(line: String)
}

/// One model row, as the owner picked it ("W1 better"): the model's name and one grey line, then, when it is not there, an amber dot, ONE line that says
/// what is missing and ONE button that fixes it in place (Download with its ↩ cap, or Retry); while it downloads the figures, a 4 pt bar and Cancel with
/// its esc cap. Used by the Laya row (Decisions) and the search model row (AI and agents).
/// The two places to swap when lane firststart-replies' `MissingSetupMark` is merged: `AmberDot` (the dot) and `AmberButton` (the button).
struct ModelSetupRow: View {
    let ax: String
    let name: String
    let suffix: String?
    let caption: String
    let phase: ModelPhase
    let buttonTitle: String
    let buttonId: String
    let cancelTitle: String
    let cancelId: String
    let download: () -> Void
    let cancel: (() -> Void)?
    var chip: String? = nil

    var body: some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s10) {
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                (Text(name).fontWeight(.semibold) + Text(suffix.map { " · \($0)" } ?? "")).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                Text(caption).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                switch phase {
                case .ready: EmptyView()
                case .missing(let line): warnLine(line, buttonTitle)
                case .failed(let line): warnLine(line, buttonTitle)
                case .downloading(let line, let fraction):
                    Text(line).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).contentTransition(.numericText())
                    ProgressBar(fraction: fraction)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if case .ready = phase, let chip { AIChip(text: chip, tone: .good) }
            if case .downloading = phase, let cancel {
                SettingsButton(title: cancelTitle, key: "esc", ax: cancelId, action: cancel).keyboardShortcut(.cancelAction)
            }
        }
        .padding(.horizontal, SettingsMetrics.rowSideX).padding(.vertical, WhooshSpacing.s10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: phase)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(ax)
    }

    private func warnLine(_ line: String, _ button: String) -> some View {
        HStack(spacing: WhooshSpacing.s8) {
            AmberDot()
            Text(line).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.warning).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: WhooshSpacing.s8)
            AmberButton(title: button, key: "\u{21A9}", ax: buttonId, action: download).keyboardShortcut(.defaultAction)
        }
        .padding(.top, WhooshSpacing.s6)
    }
}

/// The 7 pt amber dot with its soft halo (board `.wd`).
struct AmberDot: View {
    var body: some View {
        Circle().fill(WhooshPalette.warning).frame(width: 7, height: 7)
            .overlay(Circle().stroke(WhooshPalette.warning.opacity(0.24), lineWidth: 3))
            .padding(3).accessibilityHidden(true)
    }
}

/// The amber button with its key cap (board `.b2.amb`): the warn fill, dark text.
struct AmberButton: View {
    let title: String
    let key: String
    let ax: String
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s6) {
                Text(title).font(.system(size: 12.5, weight: .semibold)).lineLimit(1)
                Text(key).font(.system(size: 10.5, weight: .semibold)).padding(.horizontal, WhooshSpacing.s4).frame(minWidth: 16, minHeight: 16)
                    .background(RoundedRectangle(cornerRadius: 4, style: .continuous).fill(Color.black.opacity(0.14)))
            }
            .foregroundStyle(Color.black.opacity(0.88))
            .padding(.horizontal, 10).frame(height: 26).fixedSize()
            .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(WhooshPalette.warning))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .whooshAx(ax)
    }
}

/// The 4 pt bar of a download (board `.bar`); no figure known yet shows an empty track.
struct ProgressBar: View {
    let fraction: Double?
    var body: some View {
        GeometryReader { g in
            ZStack(alignment: .leading) {
                Capsule().fill(WhooshPalette.fill)
                Capsule().fill(WhooshPalette.accent).frame(width: g.size.width * CGFloat(min(max(fraction ?? 0, 0), 1)))
            }
        }
        .frame(height: 4).padding(.top, WhooshSpacing.s6)
        .accessibilityHidden(true)
    }
}
