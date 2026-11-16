import SwiftUI

/// The accounts added in this session: a small grey list under the added state, and Welcome's "Added" list with what each one is doing.
struct AddAccountSessionView: View {
    let model: AddAccountModel

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(AddAccountWords.sessionCap).whooshText(WhooshTypography.caption, emphasized: true)
                Spacer(minLength: 0)
                Text("\(model.added.count)").whooshText(WhooshTypography.caption)
            }
            .foregroundStyle(WhooshPalette.secondaryLabel)
            AddAccountFlowLayout(hSpacing: 14, vSpacing: 6) {
                ForEach(model.added) { a in
                    HStack(spacing: 6) {
                        MarkChip(kind: a.kind, diameter: AddAccountMetrics.sessionChip)
                        Text(a.kind.name).whooshText(WhooshTypography.subhead)
                    }
                    .foregroundStyle(a.id == model.added.last?.id ? WhooshPalette.label : WhooshPalette.secondaryLabel)
                }
            }
        }
        .padding(.top, 12).overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: 1) }
        .padding(.top, 16)
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(AddAccountWords.surface).accounts")
    }
}

/// Welcome's list under the card: each added account with its mark, name, address, and what it is doing now.
struct AddAccountAddedList: View {
    let model: AddAccountModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(AddAccountWords.addedCap).whooshText(WhooshTypography.caption, emphasized: true)
                Spacer(minLength: 0)
                Text("\(model.added.count)").whooshText(WhooshTypography.caption)
            }
            .foregroundStyle(WhooshPalette.secondaryLabel).padding(.top, 12).padding(.bottom, 4)
            TimelineView(.periodic(from: .now, by: 1)) { _ in
                ForEach(model.added) { a in
                    HStack(spacing: 10) {
                        MarkChip(kind: a.kind, diameter: AddAccountMetrics.rowChip)
                        Text(a.name).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                        Text(a.address).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                        Spacer(minLength: 8)
                        progress(AddAccountRows.progress(a))
                    }
                    .frame(height: 36).overlay(alignment: .bottom) { Rectangle().fill(WhooshPalette.separator).frame(height: 1) }
                }
            }
        }
        .padding(.top, 18).overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: 1).padding(.top, 18) }
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(AddAccountWords.surface).accounts")
    }

    @ViewBuilder private func progress(_ p: String) -> some View {
        HStack(spacing: 6) {
            if p == AddAccountWords.ready { Image(systemName: "checkmark").font(.system(size: 11, weight: .semibold)) } else { ProgressView().controlSize(.small).scaleEffect(0.6).frame(width: 12, height: 12) }
            Text(p == AddAccountWords.ready ? AddAccountWords.readyWords : (p == AddAccountWords.connecting ? AddAccountWords.connectingWords : (p == AddAccountWords.fetching ? AddAccountWords.fetchingWords : p)))
                .whooshText(WhooshTypography.subhead)
        }
        .foregroundStyle(WhooshPalette.secondaryLabel)
    }
}

/// Items in a row that wrap to the next line, gaps as the board's `flex-wrap`.
struct AddAccountFlowLayout: Layout {
    var hSpacing: CGFloat, vSpacing: CGFloat
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize { place(proposal.width ?? 10_000, subviews, nil) }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) { _ = place(bounds.width, subviews, bounds.origin) }
    private func place(_ width: CGFloat, _ subviews: Subviews, _ origin: CGPoint?) -> CGSize {
        var x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0, maxX: CGFloat = 0
        for s in subviews {
            let sz = s.sizeThatFits(.unspecified)
            if x > 0, x + sz.width > width { x = 0; y += row + vSpacing; row = 0 }
            if let o = origin { s.place(at: CGPoint(x: o.x + x, y: o.y + y), proposal: ProposedViewSize(sz)) }
            x += sz.width + hSpacing; row = max(row, sz.height); maxX = max(maxX, x - hSpacing)
        }
        return CGSize(width: maxX, height: y + row)
    }
}
