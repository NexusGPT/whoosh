import SwiftUI

/// The sender-domain filter (SPEC A153) on the phone. Typing letters that start a sender's domain offers "From domain <domain>" with its number of
/// people; choosing it makes the pill "From @<domain>" and the list keeps the mail whose sender's address ends with that domain. The offers are the
/// Mac's own rows (`SearchSuggest.build` on `DomainRead.domains`, the engine's `senderDomains(prefix:)`), read here and not worked out again.
@MainActor
extension SearchPhoneModel {
    /// Reads the offers for the word being typed (the text must end in it: a space after it ends the offer). Runs with each search turn, never in the key's frame.
    func readDomainOffers() {
        let typed = app.typed
        guard !naming, let last = typed.split(whereSeparator: \.isWhitespace).last, typed.last?.isWhitespace == false,
              !DomainRead.letters(String(last)).isEmpty else { domainOffers = []; return }
        let found = DomainRead.domains(core: app.core, prefix: String(last))
        let source = SearchSuggest.Source(people: [], labels: [], folders: [], recents: [], domains: found)
        domainOffers = SearchSuggest.build(draft: typed, chips: [], source: source, recentMax: 0).filter { $0.kind == .domain }
    }

    /// The skeleton hold (socket only, as the Mac's): the first answer stays unlisted and the skeleton stays until the release.
    @discardableResult func hold() -> Bool { guard active else { return false }; holdSkeleton = true; dirty = true; settle(); return true }
    @discardableResult func release() -> Bool { guard active, holdSkeleton else { return false }; holdSkeleton = false; dirty = true; settle(); return true }

    /// A tap on an offer: the pill "From @<domain>" is set, the typed word it stood for leaves the field, the list narrows on the same frame.
    @discardableResult
    func chooseDomain(_ index: Int) -> Bool {
        guard domainOffers.indices.contains(index), let chip = domainOffers[index].chip, chip.kind == .domain else { return false }
        let word = domainOffers[index].replaces ?? ""
        filters.domain = chip.value
        var typed = app.typed.trimmingCharacters(in: .whitespaces)
        if !word.isEmpty, typed.hasSuffix(word) { typed.removeLast(word.count) }
        app.typed = typed.trimmingCharacters(in: .whitespaces)
        domainOffers = []; menu = nil
        filtersChanged()
        return true
    }
}

/// "@ From domain <domain>   3 people": a glass card over the chips, 44 pt lines, the typed letters of the domain in bold. A tap makes the pill.
struct SearchPhoneDomainCard: View {
    let model: SearchPhoneModel

    var body: some View {
        VStack(spacing: 0) {
            ForEach(Array(model.domainOffers.enumerated()), id: \.element.id) { i, o in
                Button { _ = model.chooseDomain(i) } label: {
                    HStack(spacing: WhooshSpacing.s8) {
                        Text(verbatim: "@").phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.secondaryLabel)
                        Text(Words.text("From domain")).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1).fixedSize()
                        Text(Self.title(o)).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label).lineLimit(1).truncationMode(.middle)
                        Spacer(minLength: WhooshSpacing.s8)
                        Text(o.detail).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.tertiaryLabel).lineLimit(1).fixedSize()
                    }
                    .padding(.horizontal, PhoneMetrics.gutter).frame(minHeight: PhoneMetrics.mailboxRow).contentShape(Rectangle())
                }
                .buttonStyle(SearchRowPressStyle())
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("iphone-search.domain.\(i)")
            }
        }
        .padding(.vertical, WhooshSpacing.s4)
        .whooshGlass(in: RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
        .padding(.horizontal, PhoneMetrics.gutter)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("iphone-search.domains")
    }

    /// The domain with the letters typed so far in bold (the suggestion's own `bold` range).
    static func title(_ o: SearchSuggestion) -> AttributedString {
        var a = AttributedString(o.title)
        if let b = o.bold, b.upperBound <= o.title.count, !b.isEmpty {
            let lo = a.index(a.startIndex, offsetByCharacters: b.lowerBound), hi = a.index(lo, offsetByCharacters: b.count)
            a[lo..<hi].inlinePresentationIntent = .stronglyEmphasized
        }
        return a
    }
}
