import SwiftUI

/// One account's card (`.acard`): its ring, its address, « 4 dossiers sur 6 · 18 096 messages », the folder syncing now, then its folders as
/// tiles. Closed, it is one row « 2 dossiers sur 5 › » that opens it (each account keeps its own sources, owner round 7).
struct AccountSourcesCard: View {
    let account: ReplyAccountSources
    @State private var store = RepliesStore.shared
    private var P: String { RepliesAx.id("sources", "account", account.accountId) }

    var body: some View {
        let entry = AccountBook.shared.account(account.accountId)
        let name = entry?.address ?? account.accountId
        let ring = settingsColor(hex: entry?.colour)
        if store.openAccounts.contains(account.accountId) {
            VStack(alignment: .leading, spacing: WhooshSpacing.s10) {
                Button { store.toggleAccount(account.accountId) } label: {
                    HStack(spacing: WhooshSpacing.s10) {
                        AccountRing(colour: ring, size: 26)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(name).font(.system(size: 13.5, weight: .semibold)).foregroundStyle(WhooshPalette.label).lineLimit(1)
                            Text(RepliesWords.foldersLine(account)).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel).contentTransition(.numericText())
                        }
                        Spacer(minLength: WhooshSpacing.s8)
                        syncing
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain).whooshAx(P)
                SourceTileGrid {
                    ForEach(Array(account.folders.enumerated()), id: \.element.source) { i, f in SourceTile(source: f, ax: "\(P).folder.\(i)") }
                }
            }
            .padding(WhooshSpacing.s12)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(WhooshPalette.fill))
            .overlay(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
            .transition(.opacity)
        } else {
            GroupedCard {
                SettingsRow(ax: P, title: name, leading: AnyView(AccountRing(colour: ring, size: 22)), onTap: { store.toggleAccount(account.accountId) }) {
                    RepliesValue(text: RepliesWords.foldersOf(account.enabledFolders, account.totalFolders))
                }
            }
        }
    }

    /// « ◔ Clients, 62 % »: the first folder of the account still syncing.
    @ViewBuilder private var syncing: some View {
        if let f = account.folders.first(where: { if case .syncing = $0.progress { return true } else { return false } }), case .syncing(let p) = f.progress {
            HStack(spacing: WhooshSpacing.s6) {
                ProgressRing(percent: Int(p))
                Text("\(RepliesWords.folderName(f.source)), \(RepliesWords.percent(p))").font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel).contentTransition(.numericText())
            }
            .whooshAx("\(P).syncing")
        }
    }
}

/// `.ftile`: a folder you select. Its symbol, its name, a tick when on; its count and its sync state under. On, it wears the accent tint.
struct SourceTile: View {
    let source: ReplySource
    let ax: String
    @State private var store = RepliesStore.shared
    @State private var hover = false  // local-ui: hover tint only
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let on = source.enabled, gone = source.isGone
        Button { store.toggleSource(source) } label: {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 7) {
                    Image(systemName: "folder").font(.system(size: 13)).foregroundStyle(on ? WhooshPalette.accent : WhooshPalette.tertiaryLabel).frame(width: 16)
                    Text(RepliesWords.folderName(source.source)).font(.system(size: 12.5, weight: .medium))
                        .foregroundStyle(on ? WhooshPalette.label : WhooshPalette.secondaryLabel)
                        .lineLimit(2).fixedSize(horizontal: false, vertical: true)   // round 24: a name wraps, never cut (two tiles read « Clients… » at 700x480)
                        .whooshAx("\(ax).name")
                    Spacer(minLength: 0)
                    // on: the accent tick. Gone: the tick where the owner's remembered choice puts it, hollow and grey (it acts on the return only)
                    let wished = gone && store.sources.wish(source) == true
                    Image(systemName: wished ? "checkmark.circle" : "checkmark.circle.fill").font(.system(size: 14))
                        .foregroundStyle(wished ? WhooshPalette.secondaryLabel : WhooshPalette.accent)
                        .opacity(on || wished ? 1 : 0).scaleEffect(on || wished ? 1 : 0.6)
                        .whooshAx("\(ax).tick")
                }
                if case .localFolder(let path) = source.source {
                    Text(path).font(.system(size: 11)).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1).truncationMode(.middle).padding(.leading, 23)
                        .whooshAx("\(ax).path")
                }
                HStack {
                    Text(RepliesWords.count(source)).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel)
                    Spacer(minLength: 4)
                    Text(on ? RepliesWords.progress(source.progress) : "").font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel)
                }
                .contentTransition(.numericText()).padding(.leading, 9).padding(.top, 2)
                .frame(maxHeight: .infinity, alignment: .bottom)   // a tile made taller by its row's tallest keeps its count line at the bottom, level with its neighbours'
                if gone {
                    // delta 24: the server no longer holds it; off, its last count, and one plain line
                    Text(RepliesWords.goneLine).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel)
                        .lineLimit(1).padding(.leading, 9).transition(.opacity)
                        .whooshAx("\(ax).gone")
                }
            }
            .opacity(gone ? 0.55 : 1)
            .padding(.horizontal, 11).padding(.top, 9).padding(.bottom, 8)
            .frame(maxHeight: .infinity, alignment: .top)   // as tall as its row (SourceTileRows)
            .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(fill(on)))
            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous)
                .strokeBorder(on ? WhooshPalette.accent.color(scheme).opacity(0.55) : WhooshPalette.separator.color(scheme), lineWidth: on ? 1 : 0.5))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain).onHover { hover = $0 }
        .animation(WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion), value: on)
        .animation(WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion), value: gone)
        .help(gone ? RepliesWords.goneHelp : "")
        .animation(.easeOut(duration: WhooshMotion.hoverTint), value: hover)
        .accessibilityLabel(RepliesWords.folderName(source.source)).accessibilityAddTraits(on ? .isSelected : [])
        .accessibilityValue(gone ? RepliesWords.goneLine : "")
        .whooshAx(ax)
    }

    private func fill(_ on: Bool) -> Color {
        let accent = WhooshPalette.accent.color(scheme)
        if on { return accent.opacity(scheme == .dark ? 0.16 : 0.09) }
        if hover { return WhooshPalette.hover.color(scheme) }
        return scheme == .dark ? Color.white.opacity(0.04) : WhooshPalette.background.color(scheme)
    }
}

/// `.pring`: a 16 pt ring that fills as the folder syncs.
struct ProgressRing: View {
    let percent: Int
    var body: some View {
        ZStack {
            Circle().stroke(WhooshPalette.fill, lineWidth: 3)
            Circle().trim(from: 0, to: CGFloat(percent) / 100).stroke(WhooshPalette.accent, style: StrokeStyle(lineWidth: 3, lineCap: .round)).rotationEffect(.degrees(-90))
        }
        .frame(width: 13, height: 13).padding(1.5)
        .animation(.easeOut(duration: WhooshMotion.readerCrossFade), value: percent)
        .accessibilityLabel("\(percent) %")
    }
}

/// `.ring`: an account's colour as a hollow ring.
struct AccountRing: View {
    let colour: Color
    let size: CGFloat
    var body: some View {
        Circle().strokeBorder(colour, lineWidth: 1.5).frame(width: size, height: size).accessibilityHidden(true)
    }
}
