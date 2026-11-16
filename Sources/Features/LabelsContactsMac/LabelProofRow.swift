import SwiftUI

/// One matched message in pane 2: the list row's own geometry (44 pt, emblem column, two lines) with the rule tag in the glyph
/// slot, the matched word faintly highlighted, and, on a re-run preview, where the message goes: a small dot in the leading slot
/// (join blue, leave and stay grey) and the word in the meta line (joins, leaves, stays). No strip on the row's edge (owner, 15:46).
struct ProofRowView: View {
    let row: ProofRow
    let account: RowAccount
    var stale = false
    /// AI sorting is on for the label: a row the rules pick is "picked" (the AI has not ruled yet), not "joins"
    var aiOn = false
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s6) {
                Text(row.senderName).font(WhooshTypography.body.font(emphasized: row.unread)).foregroundStyle(row.edge == .leave ? WhooshPalette.secondaryLabel : WhooshPalette.label).lineLimit(1)
                if row.threadCount > 1 { Text("\(row.threadCount)").font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel) }
                Spacer(minLength: WhooshSpacing.s8)
                if let e = row.edge {
                    Text(Self.word(e, aiOn: aiOn)).font(WhooshTypography.caption.font(emphasized: e == .join))
                        .foregroundStyle(e == .join ? WhooshPalette.accent : WhooshPalette.secondaryLabel).fixedSize()
                        .accessibilityIdentifier("mac-labels.inspector.row.\(row.id).edge.\(e.rawValue)")
                }
                Text(RowFacts.time(row.receivedAtMs)).font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize()
            }
            .frame(height: WhooshMetrics.listRowLine).padding(.top, WhooshSpacing.s4)
            HStack(spacing: WhooshSpacing.s6) {
                Text(subject).font(WhooshTypography.subhead.font(emphasized: false))
                    .foregroundStyle(row.unread && row.edge != .leave ? WhooshPalette.label : WhooshPalette.secondaryLabel).lineLimit(1)
                Spacer(minLength: WhooshSpacing.s4)
                if !row.tag.isEmpty { tag }
            }
            .frame(height: WhooshMetrics.listRowLine)
        }
        .padding(.leading, WhooshMetrics.listRowLeading + WhooshSpacing.s4).padding(.trailing, WhooshSpacing.s12)
        .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRow, alignment: .leading)
        .overlay(alignment: .topLeading) { marks }
        .overlay(alignment: .bottom) {
            Rectangle().fill(WhooshPalette.separator).frame(height: 1).padding(.leading, WhooshMetrics.listRowInset * 5).padding(.trailing, WhooshMetrics.listRowInset)
        }
        .padding(.horizontal, WhooshMetrics.listRowInset)
        .opacity(stale ? 0.5 : 1)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(row.senderName), \(row.subject), \(row.tag)")
        .whooshAx("mac-labels.inspector.row.\(row.id)")
    }

    private var tag: some View {
        Text(row.tag).font(.system(size: 10, weight: .medium)).foregroundStyle(row.wasTag ? WhooshPalette.tertiaryLabel : WhooshPalette.secondaryLabel)
            .lineLimit(1).padding(.horizontal, WhooshSpacing.s6).frame(height: 16)
            .overlay { Capsule().strokeBorder(WhooshPalette.separator, lineWidth: 1) }
            .transition(.opacity).animation(WhooshMotion.hover, value: row.tag)
    }

    private var marks: some View {
        HStack(spacing: WhooshSpacing.s4) {
            VStack(spacing: WhooshSpacing.s4) {
                Circle().fill(row.unread ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(.clear)).frame(width: WhooshMetrics.unreadDot, height: WhooshMetrics.unreadDot)
                if let e = row.edge {
                    Circle().strokeBorder(e == .join ? WhooshPalette.accent : WhooshPalette.tertiaryLabel, lineWidth: WhooshMetrics.separator * 2)
                        .frame(width: WhooshMetrics.unreadDot, height: WhooshMetrics.unreadDot).accessibilityHidden(true)
                }
            }
            EmblemMark(glyph: EmblemLibrary.resolve(account.iconId), colourHex: account.colour, size: .row,
                       state: account.needsAttention ? .needsSignIn : .live, name: account.name, address: account.address)
        }
        .padding(.leading, WhooshSpacing.s4 + WhooshMetrics.listRowInset).padding(.top, WhooshSpacing.s4 + WhooshSpacing.s2)
    }

    /// The edge in words, through the catalogue: joins, leaves, stays.
    static func word(_ e: ProofEdge, aiOn: Bool = false) -> String {
        switch e {
        case .join: return aiOn ? Words.text("picked") : Words.text("joins")
        case .leave: return Words.text("leaves")
        case .stay: return Words.text("stays")
        }
    }

    /// The subject with the matched word faintly highlighted (the lock's accent at a low opacity).
    private var subject: AttributedString {
        var a = AttributedString(row.subject)
        if let h = row.highlight, let r = a.range(of: h, options: [.caseInsensitive, .diacriticInsensitive]) { a[r].backgroundColor = WhooshPalette.accent.color(scheme).opacity(0.14) }
        return a
    }
}

/// A rule that matches nothing, said plainly, with the nearest sender domain the mailboxes know when one is close.
struct ProofEmpty: View {
    @Bindable var model: LabelInspectorModel

    var body: some View {
        let typed = model.draft.rules.first { $0.kind == .domain && !$0.allValues.isEmpty }
        let ruleless = model.draft.rules.allSatisfy(\.isEmpty)
        let near = typed.flatMap { r in r.allValues.last.flatMap { DomainNearness.closest(to: $0, in: model.corpus.map(\.facts.fromAddress)) } }
        VStack(spacing: 0) {
            Image(systemName: "tag").font(.system(size: 30, weight: .light)).foregroundStyle(WhooshPalette.tertiaryLabel).padding(.bottom, WhooshSpacing.s12)
            if ruleless, let why = model.aiRefusalShown {
                // no rule and the AI cannot sort: say why, and open the pane where it is fixed (replaces "No rule yet ... files mail by its meaning")
                Text(why).whooshText(WhooshTypography.title3).foregroundStyle(WhooshPalette.label).multilineTextAlignment(.center).frame(maxWidth: 320)
                    .accessibilityIdentifier("mac-labels.inspector.empty.why")
                if let door = model.refusalDoor {
                    Button { _ = model.runRefusalDoor(door.id) } label: {
                        Text(door.title).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent).padding(.horizontal, WhooshSpacing.s12)
                            .frame(height: 30).background(WhooshPalette.fill.opacity(0.7), in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
                    }.buttonStyle(.plain).padding(.top, WhooshSpacing.s14).accessibilityIdentifier("mac-labels.inspector.\(door.id)")
                }
            } else {
            Text(ruleless ? Words.text("No rule yet") : Words.text("Nothing matches yet")).whooshText(WhooshTypography.title3).foregroundStyle(WhooshPalette.label)
            Text(ruleless ? Words.text("Add a rule to see the mail it catches.")
                 : Words.text("No mail in your %lld accounts matches these rules.", AccountBook.shared.accounts(core: model.core).count))
                .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).multilineTextAlignment(.center).frame(maxWidth: 290).padding(.top, WhooshSpacing.s6)
            if let near, let r = typed {
                Text(Words.text("Did you mean")).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).padding(.top, WhooshSpacing.s16)
                HStack(spacing: WhooshSpacing.s8) {
                    Text(near.domain).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.label)
                    Text(near.count == 1 ? Words.text("1 message") : Words.text("%lld messages", near.count)).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    Button { model.editRule(r.id) { $0.pending = ""; $0.values = [near.domain] } } label: {
                        Text(Words.text("Use it")).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent).padding(.horizontal, WhooshSpacing.s6)
                    }.buttonStyle(.plain).help(Words.text("Use %@", near.domain)).accessibilityIdentifier("mac-labels.inspector.useclosest")
                }
                .padding(.leading, WhooshSpacing.s12).padding(.trailing, WhooshSpacing.s6).frame(height: 30)
                .background(WhooshPalette.fill.opacity(0.7), in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous)).padding(.top, WhooshSpacing.s6)
            }
            Text(Words.text("You can still create it. The decision model files mail by its meaning.")).whooshText(WhooshTypography.subhead)
                .foregroundStyle(WhooshPalette.secondaryLabel).multilineTextAlignment(.center).frame(maxWidth: 290).padding(.top, WhooshSpacing.s14)
            }
        }
        .padding(.top, WhooshSpacing.s32 * 2).padding(.horizontal, WhooshSpacing.s32).frame(maxWidth: .infinity)
        .accessibilityIdentifier("mac-labels.inspector.empty")
    }
}

/// The row that is still coming while a count is read: three grey bars, no spinner on the list.
struct Skeleton: View {
    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            Capsule().fill(WhooshPalette.fill).frame(width: 140, height: 9)
            Capsule().fill(WhooshPalette.fill).frame(width: 230, height: 9)
        }
        .padding(.leading, WhooshMetrics.listRowLeading + WhooshSpacing.s12).frame(height: WhooshMetrics.listRow, alignment: .center)
        .accessibilityHidden(true)
    }
}

/// Which known sender domain is nearest to a typed one: within two edits, the most mail first.
enum DomainNearness {
    static func closest(to typed: String, in addresses: [String]) -> (domain: String, count: Int)? {
        let t = typed.lowercased()
        var counts: [String: Int] = [:]
        for a in addresses { if let d = a.split(separator: "@").last { counts[String(d).lowercased(), default: 0] += 1 } }
        return counts.filter { $0.key != t && distance($0.key, t) <= 2 }.max { $0.value < $1.value }.map { ($0.key, $0.value) }
    }

    /// Levenshtein distance, two rows.
    static func distance(_ a: String, _ b: String) -> Int {
        let x = Array(a), y = Array(b)
        if x.isEmpty { return y.count }
        var prev = Array(0...y.count)
        for i in 1...x.count {
            var cur = [i]
            for j in 1...max(1, y.count) where !y.isEmpty {
                cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (x[i - 1] == y[j - 1] ? 0 : 1)))
            }
            prev = y.isEmpty ? [i] : cur
        }
        return prev[y.count]
    }
}
