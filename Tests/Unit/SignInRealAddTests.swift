import Foundation
import Testing
@testable import WhooshUI

// The sign-in route's save step against a core that refuses the address-only `.addAccount` and accepts `.addAccountConfigured`
// (RealishCore, Tests/Unit/WelcomeRealAddTests.swift). No sheet, no network, no secret. Every test carries a time limit.

@MainActor @Suite(.timeLimit(.minutes(1))) struct SignInRealAddTests {
    @Test func aSignedInAccountIsSavedWithAddAccountConfiguredAndTheDetectedServers() async throws {
        let core = RealishCore()
        let c = SignInController()
        var refreshed = 0
        await c.saveAccount(.microsoft, address: "Ana@Example.test", core: core, fixture: false, refresh: { refreshed += 1 })
        #expect(sentAddAccountCount(core) == 0)
        let sent = try #require(configuredSent(core))
        #expect(sent.address == "ana@example.test")
        #expect(sent.config.imapHost == "imap.example.test")
        #expect(sent.config.smtpHost == "smtp.example.test")
        #expect(sent.config.authMethod == .oauth2)                      // the sign-in route: the provider's sign-in, no password
        #expect(sent.passwordIs(""))
        #expect(refreshed == 1)
        #expect(c.phase == .idle)                                       // no failure phase on a saved account
    }

    @Test func detectionFailureShowsItsSentenceAndSendsNothing() async {
        let core = RealishCore(detection: .refused(words: "No mail server could be found for example.test."))
        let c = SignInController()
        var refreshed = 0
        await c.saveAccount(.google, address: "ana@example.test", core: core, fixture: false, refresh: { refreshed += 1 })
        #expect(c.phase == .failed("No mail server could be found for example.test."))
        #expect(core.commands.isEmpty)
        #expect(refreshed == 0)
    }

    @Test func theEnginesRefusalReadsAsTheCatalogueSentence() async {
        let core = RealishCore(configuredRefusal: ActionFailed(accountName: "Whoosh", reasonWords: "raw engine words", retryable: false, code: "oauth.no_refresh_token"))
        let c = SignInController()
        await c.saveAccount(.fastmail, address: "ana@example.test", core: core, fixture: false, refresh: {})
        #expect(c.phase == .failed(EngineRefusal.sentence("oauth.no_refresh_token")))
    }
}
