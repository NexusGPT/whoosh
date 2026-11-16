import SwiftUI

/// The first page: every label in tree order, with its colour, its pin and its unread, then the people. The Mailboxes screen's
/// Labels row opens it; a row pushes that label's (or that person's) page.
struct LabelsListPhone: View {
    private let m = AppModel.shared
    private let state = LabelsContactsPhoneState.shared

    var body: some View {
        let _ = state.revision
        let _ = m.mailRevision          // the unread figures are read from the core here: drawn again when the mail changes
        let rows = m.core.labels()
        let tree = LabelTree(rows: rows)
        let byId = Dictionary(rows.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        let people = PhoneContactReader.people(m)
        let folders = m.core.smartFolders()
        ZStack(alignment: .top) {
            PhoneGround.grouped.ignoresSafeArea()
            ScrollView {
                VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
                    PhoneLargeTitle(text: LabelsWords.labels)
                    PhoneSectionTitle(LabelsWords.labels, trailing: "\(rows.count)")
                    PhoneGroupCard {
                        if rows.isEmpty {
                            Text(Words.text("No labels yet.")).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                                .padding(PhoneMetrics.gutter).frame(maxWidth: .infinity, alignment: .leading)
                        }
                        ForEach(Array(tree.ordered().enumerated()), id: \.element.id) { i, item in
                            if i > 0 { PhoneHair(inset: PhoneMetrics.gutter + LabelsPhoneMetrics.colourDot + WhooshSpacing.s12) }
                            if let l = byId[item.id] { labelRow(l, depth: tree.depth(item.id)) }
                        }
                    }
                    if !folders.isEmpty {
                        PhoneSectionTitle(LabelsWords.smartFolders, trailing: "\(folders.count)")
                        PhoneGroupCard {
                            ForEach(Array(folders.enumerated()), id: \.element.id) { i, f in
                                if i > 0 { PhoneHair() }
                                smartFolderRow(f, index: i)
                            }
                        }
                    }
                    if !people.isEmpty {
                        PhoneSectionTitle(LabelsWords.people, trailing: "\(people.count)")
                        PhoneGroupCard {
                            ForEach(Array(people.enumerated()), id: \.element.id) { i, p in
                                if i > 0 { PhoneHair() }
                                personRow(p, index: i)
                            }
                        }
                    }
                }
                .padding(.bottom, PhoneMetrics.listBottomRoom)
            }
            .scrollIndicators(.hidden)
        }
        .overlay(alignment: .topLeading) { PhoneBackButton { state.back(m) } }
        .phoneBarHidden()
    }

    private func labelRow(_ l: MailLabel, depth: Int) -> some View {
        Button { state.push(.label(l.id)) } label: {
            HStack(spacing: WhooshSpacing.s12) {
                Circle().fill(LabelColourPhone.style(l.colour)).frame(width: LabelsPhoneMetrics.colourDot, height: LabelsPhoneMetrics.colourDot)
                    .accessibilityHidden(true)
                Text(l.name).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Spacer(minLength: WhooshSpacing.s8)
                if l.pinned { Image(systemName: "pin.fill").font(.system(size: PhoneType.footnote.size)).foregroundStyle(WhooshPalette.accent) }
                if l.unread > 0 { Text("\(l.unread)").foregroundStyle(WhooshPalette.sidebarSecondaryLabel) }
                Image(systemName: "chevron.right").font(.system(size: PhoneType.footnote.size, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
            }
            .phoneText(PhoneType.body)
            .padding(.leading, PhoneMetrics.gutter + CGFloat(max(0, depth - 1)) * LabelsPhoneMetrics.nest).padding(.trailing, PhoneMetrics.gutter)
            .frame(maxWidth: .infinity, minHeight: PhoneMetrics.mailboxRow).contentShape(Rectangle())
        }
        .buttonStyle(PhoneRowPress())
        .accessibilityIdentifier("iphone-labels-contacts.list.label.\(l.id)")
    }

    private func personRow(_ p: Contact, index: Int) -> some View {
        Button { if let a = p.addresses.first { state.push(.contact(a)) } } label: {
            HStack(spacing: WhooshSpacing.s12) {
                VStack(alignment: .leading, spacing: 0) {
                    Text(p.name).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
                    Text(p.addresses.first ?? "").phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel).lineLimit(1)
                }
                Spacer(minLength: WhooshSpacing.s8)
                Image(systemName: "chevron.right").font(.system(size: PhoneType.footnote.size, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
            }
            .padding(.horizontal, PhoneMetrics.gutter)
            .frame(maxWidth: .infinity, minHeight: PhoneMetrics.accountRow).contentShape(Rectangle())
        }
        .buttonStyle(PhoneRowPress())
        .accessibilityIdentifier("iphone-labels-contacts.list.person.\(index)")
    }

    /// A smart folder, read only (its rule is edited on the Mac): the name, the rule in the core's words, the folder's count.
    private func smartFolderRow(_ f: SmartFolder, index: Int) -> some View {
        HStack(spacing: WhooshSpacing.s12) {
            VStack(alignment: .leading, spacing: 0) {
                Text(f.name).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Text(f.ruleWords).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel).lineLimit(2)
            }
            Spacer(minLength: WhooshSpacing.s8)
            if let n = SidebarData.smartFolderCount(f.id, model: m) { Text("\(n)").phoneText(PhoneType.body).foregroundStyle(WhooshPalette.sidebarSecondaryLabel) }
        }
        .padding(.horizontal, PhoneMetrics.gutter)
        .frame(maxWidth: .infinity, minHeight: PhoneMetrics.accountRow, alignment: .leading)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("iphone-labels-contacts.smart-folders.\(index)")
    }
}
