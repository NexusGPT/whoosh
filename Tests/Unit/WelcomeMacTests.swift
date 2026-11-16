import Foundation
import CoreImage
import Testing
@testable import WhooshUI

// Written from docs/JOURNEYS.md, docs/CONSTANTS.md and the fixture, never from a built screen. A test of a feature
// that is still a stub FAILS on purpose: the failure is the honest state until the lane lands.
// Run with WHOOSH_FIXTURE_CORE=1 so the shared AppModel serves the design fixture.
// MUTANTS: each test names, in its trailing comment, the mutation of the target that must turn it red.

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
@MainActor private func rows(_ ax: String) throws -> [ControlRow] {
    let src = try #require(Registrations.rowSource(ax), "no rows are registered at \(ax) yet")
    return try src(try model())
}
private func fixture() throws -> [String: Any] {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let data = try Data(contentsOf: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"))
    return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
}
private func list(_ key: String) throws -> [[String: Any]] { try #require(try fixture()[key] as? [[String: Any]]) }

@MainActor @Test func theWelcomeSurfaceIsDrawnByTheWelcomeLane() {
    #expect(WelcomeMacFeature.surfaces.contains { $0.hasPrefix("mac-welcome") })    // mutant: surface list empty
}
@MainActor @Test func slice1WelcomeServesImapRoutesOnly() throws {
    // The app serves one `route` row for the address typed so far (WelcomeRows.route, WelcomeMacFeature.swift), not a list of routes: type an address from a host nobody special-cases and read its route.
    _ = try model()
    let welcome = WelcomeModel.shared
    welcome.address = "someone@example.org"
    defer { welcome.address = "" }
    let routes = try rows("mac-welcome-accounts.route").compactMap { $0["route"] as? String }
    #expect(routes == ["imap"])                    // mutant: IMAP route missing, or another route served for an unknown host
    #expect(!routes.isEmpty)                       // positive control
}

// MARK: routes, paste, refusals, order and the pairing code (gap-welcome). The model gets a private UserDefaults suite and the fake connector:
// no account is touched and nothing leaves this Mac. Each test names, in its trailing comment, the mutation that must turn it red.

@MainActor private func freshWelcome() throws -> (WelcomeModel, UserDefaults, String) {
    _ = try model()
    let suite = "whoosh.test.welcome.\(UUID().uuidString)"
    let d = try #require(UserDefaults(suiteName: suite))
    let w = WelcomeModel(defaults: d)
    w.connector = WelcomeFakeConnector()
    return (w, d, suite)
}

@MainActor @Test(.timeLimit(.minutes(1))) func theGoogleRouteShowsItsAppPasswordStepsAndLink() {
    let r = WelcomeRoutes.detect("x@gmail.com")
    #expect(r.kind == .google)
    #expect(r.steps.count == 3)                                                  // mutant: WelcomeRoutes.google loses a step
    #expect(r.link == "https://myaccount.google.com/apppasswords")               // mutant: the link points at another page
    #expect(!r.usesSheet)
}

@MainActor @Test(.timeLimit(.minutes(1))) func aRefusedHostSaysWrongPasswordOr2StepIsOff() {
    let words = WelcomeRoutes.google.refusedWords()
    #expect(words == Words.text("wrong password, or 2-step sign-in is off at %@", "Google"))   // mutant: the refusal line names no cause
    if Words.current == "en" { #expect(words.contains("wrong password, or 2-step sign-in is off")) }
    #expect(WelcomeRoutes.google.refusedWords("the host said no") == "the host said no")           // the server's own words win
}

@MainActor @Test(.timeLimit(.minutes(1))) func aPastedListOfFiveAddressesMakesFiveRows() throws {
    let (w, d, suite) = try freshWelcome(); defer { d.removePersistentDomain(forName: suite) }
    w.pasteText = "a1@example.org, a2@example.org; a3@example.org\na4@example.org a5@example.org"
    #expect(w.addPasted() == 5)                                                  // mutant: addPasted keeps only the first address
    #expect(w.rows.count == 5)
    #expect(w.rows.allSatisfy { $0.phase == .waiting })
    #expect(w.pasteText.isEmpty && !w.pasteOpen)
}

@MainActor @Test(.timeLimit(.minutes(1))) func oneRefusedRowShowsItsReasonWhileTheOthersProgress() async throws {
    let (w, d, suite) = try freshWelcome(); defer { d.removePersistentDomain(forName: suite) }
    w.pasteText = "b1@example.org b2@example.org refused@example.org b4@example.org b5@example.org"
    #expect(w.addPasted() == 5)
    #expect(w.connectWaiting() == 5)
    await w.settle()
    let refused = w.rows.filter { $0.phase == .refused }
    #expect(refused.map(\.address) == ["refused@example.org"])                  // mutant: the fake refuses nothing, or every row
    #expect(refused.first?.status.isEmpty == false)
    #expect(refused.first?.status == WelcomeRoutes.detect("refused@example.org").refusedWords())
    #expect(w.rows.filter { $0.phase != .refused }.count == 4)
    #expect(w.rows.filter { $0.phase != .refused }.allSatisfy { $0.phase == .healthy })   // the other four carried on
}

@MainActor @Test(.timeLimit(.minutes(1))) func theFourRoutesOfRecord2MakeFourHealthyRows() async throws {
    let (w, d, suite) = try freshWelcome(); defer { d.removePersistentDomain(forName: suite) }
    w.pasteText = "one@gmail.com one@outlook.com one@icloud.com one@fastmail.com"
    #expect(w.addPasted() == 4)
    #expect(w.rows.map(\.routeName) == ["Google", "Microsoft", "iCloud", "Fastmail"])
    #expect(w.connectWaiting() == 4)
    await w.settle()
    #expect(w.healthyCount == 4)                                                 // mutant: connectWaiting skips a row
    #expect(w.rows.allSatisfy { $0.phase == .healthy })
}

@MainActor @Test(.timeLimit(.minutes(1))) func anAdminBlockedMicrosoftAddressShowsTheAdminLine() async throws {
    let (w, d, suite) = try freshWelcome(); defer { d.removePersistentDomain(forName: suite) }
    w.pasteText = "admin-blocked@outlook.com fine@outlook.com admin-blocked@gmail.com"
    #expect(w.addPasted() == 3)
    #expect(w.connectWaiting() == 3)
    await w.settle()
    let blocked = try #require(w.rows.first { $0.address == "admin-blocked@outlook.com" })
    #expect(blocked.phase == .refused)                                           // mutant: the fake has no admin-blocked rule
    #expect(blocked.status == Words.text(SignInWords.adminBlocked))
    #expect(blocked.progressWords == blocked.status)
    #expect(w.rows.first { $0.address == "fine@outlook.com" }?.phase == .healthy)
    #expect(w.rows.first { $0.address == "admin-blocked@gmail.com" }?.phase == .healthy)   // the line is Microsoft's alone
}

@MainActor @Test(.timeLimit(.minutes(1))) func thirtyRowsKeepTheirMovedOrderAfterARelaunch() throws {
    let (w, d, suite) = try freshWelcome(); defer { d.removePersistentDomain(forName: suite) }
    let addresses = (1...30).map { "row\($0)@example.org" }
    w.pasteText = addresses.joined(separator: "\n")
    #expect(w.addPasted() == 30)
    w.move(from: [0], to: 5)                                                     // the first row dragged below the fifth
    var expected = addresses
    expected.insert(expected.remove(at: 0), at: 4)
    #expect(w.rows.map(\.address) == expected)                                   // mutant: move(from:to:) does nothing
    let relaunched = WelcomeModel(defaults: d)                                   // a new model over the same store: what a relaunch reads
    #expect(relaunched.rows.map(\.address) == expected)                          // mutant: move() forgets to save
    #expect(relaunched.rows.count == 30)
}

/// The code itself, rendered and read back the way a phone's camera would: one decode, not a key list.
@MainActor @Test(.timeLimit(.minutes(1))) func theQrImageDecodesToAPairingTokenOnly() throws {
    let qr = WelcomeQR(expirySeconds: WhooshConstants.QR_EXPIRY_S, nowMs: { 1_000 })
    let image = try #require(qr.image(side: 264))
    let cg = try #require(image.cgImage(forProposedRect: nil, context: nil, hints: nil))
    let pad: CGFloat = 40                                                        // the white margin a scanner needs (the card draws one too)
    let code = CIImage(cgImage: cg)
    let canvas = CGRect(x: 0, y: 0, width: code.extent.width + 2 * pad, height: code.extent.height + 2 * pad)
    let onWhite = code.transformed(by: CGAffineTransform(translationX: pad, y: pad))
        .composited(over: CIImage(color: .white).cropped(to: canvas))
    let detector = try #require(CIDetector(ofType: CIDetectorTypeQRCode, context: nil, options: [CIDetectorAccuracy: CIDetectorAccuracyHigh]))
    let found = detector.features(in: onWhite).compactMap { ($0 as? CIQRCodeFeature)?.messageString }
    #expect(found.count == 1, "the picture holds one code")
    let text = try #require(found.first)
    #expect(text == qr.token.qrPayload)
    #expect(text.range(of: "^whoosh-pair:v1:[0-9a-f]+$", options: .regularExpression) != nil)   // mutant: PairingToken.qrPayload adds an address
    #expect(!text.contains("@") && !text.lowercased().contains("password") && !text.contains("://") && !text.contains("."))
    #expect(text.hasSuffix(qr.token.value))
}
