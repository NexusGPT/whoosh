/// Registration stub for the askphone lane (folder Sources/Features/AskPhone/), written by the root commit.
/// It stays empty on purpose: the Ask panel retired into search (docs/DESIGN.md "mac-ask retires into mac-search", SPEC A26, J5.4 "the results are the answer"),
/// so the phone answers a question with `iphone-search` and registers no citations, proposal or approve control here (Tests/Unit/AskPhoneTests.swift pins that).
public enum AskPhoneFeature: FeatureModule {
    public static let id = "askphone"
    public static let surfaces: [String] = ["iphone-ask"]
}
