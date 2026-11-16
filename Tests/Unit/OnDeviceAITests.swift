import Foundation
import Testing
@testable import WhooshUI

// OnDeviceAITests: the on-device AI lane (docs/lanes/ondeviceai.md rounds 1 to 4).
// What each test proves is the engine's own logic (which backend answers, what is refused before a call, what the
// Settings row says); the backends here are probes that count calls, never the thing under test.
// UNCOMPILED at the commit that wrote it (the main51 wave): checked with `swiftc -parse` only.

/// A backend that counts how often the model is actually called.
private final class CountingBackend: TextBackend, @unchecked Sendable {
    let name: String
    private let ready: LocalModelAvailability
    private(set) var calls = 0
    init(name: String = "Counting", ready: LocalModelAvailability = .ready) { self.name = name; self.ready = ready }
    func availability() -> LocalModelAvailability { ready }
    func respond(prompt: String, maxTokens: Int) async throws -> String { calls += 1; return "answer" }
}

private final class Clock: @unchecked Sendable { var now: TimeInterval = 1_000 }

// MARK: registration and the Settings provider row

@MainActor @Test func onDeviceAIIsRegisteredOnceUnderItsLaneId() {
    let ids = FeatureRegistry.all.map { $0.id }.filter { $0 == "ondeviceai" }
    #expect(ids == ["ondeviceai"])
    #expect(FeatureRegistry.all.map { $0.id }.filter { $0 == "settingsaimac" } == ["settingsaimac"])  // control: the probe sees another lane
}

@MainActor @Test func onDeviceAIListsLocalModelsForTheProviderPicker() {
    let regions = FeatureRegistry.all.flatMap { $0.rowSources().keys }
    #expect(regions.contains { $0.hasPrefix("mac-settings-ai.provider.local") },
            "no region lists the local models found on this Mac")
    #expect(OnDeviceAIFeature.rowSources()["mac-settings-ai.provider.local"] != nil)
}

// MARK: unavailable, in words, never a crash and never a cloud answer

@Test func everyAppleReasonHasItsOwnSentence() {
    let words = AppleReason.allCases.map(\.words)
    #expect(words.allSatisfy { !$0.isEmpty })
    #expect(Set(words).count == AppleReason.allCases.count, "two reasons share one sentence")
}

@Test func aMacWithoutAppleIntelligenceReportsUnavailableInWords() async {
    for reason in AppleReason.allCases {
        let apple = ArmedAppleBackend(reason: reason)
        let engine = OnDeviceTextEngine(backends: [apple])
        guard case .unavailable(let said) = engine.availability() else { Issue.record("\(reason) reported ready"); continue }
        #expect(said.contains(reason.words), "the Settings sentence must carry the reason for \(reason)")
        do {
            _ = try await engine.respond(prompt: "Hello", maxTokens: 16)
            Issue.record("\(reason) answered")
        } catch let e as LocalModelError {
            #expect(e == .unavailable(said))
            #expect(!e.description.isEmpty)
        } catch { Issue.record("\(reason) threw \(error), not a LocalModelError") }
    }
}

@Test func withNoBackendAtAllTheSentenceSaysNoModelIsSetUp() async {
    let engine = OnDeviceTextEngine(backends: [])
    guard case .unavailable(let said) = engine.availability() else { Issue.record("empty engine reported ready"); return }
    #expect(!said.isEmpty)
    await #expect(throws: LocalModelError.unavailable(said)) { try await engine.respond(prompt: "Hello", maxTokens: 16) }
}

@Test func anUnavailableEngineNeverFallsBackToAnotherAnswerer() async {
    // Two backends, neither ready: the call count of both stays 0 and the error is the unavailable sentence.
    let a = CountingBackend(name: "A", ready: .unavailable(reason: "A is off."))
    let b = CountingBackend(name: "B", ready: .unavailable(reason: "B is off."))
    let engine = OnDeviceTextEngine(backends: [a, b])
    do { _ = try await engine.respond(prompt: "Hello", maxTokens: 16); Issue.record("answered while unavailable") }
    catch let e as LocalModelError { #expect(e.description.contains("A is off.") && e.description.contains("B is off.")) }
    catch { Issue.record("wrong error \(error)") }
    #expect(a.calls == 0 && b.calls == 0)
}

@MainActor @Test func theWiredEngineHoldsOnlyOnDeviceBackendsAppleFirst() {
    let names = OnDeviceSetup.shared.engine.backends.map(\.name)
    #expect(names == [AppleIntelligenceBackend().name, Words.text("Language model")])
    #expect(OnDeviceSetup.shared.localModel.engine.backends.map(\.name) == names, "the LocalModel and the Settings row read different engines")
}

@Test func theFirstReadyBackendAnswersAndTheNextOneIsNotCalled() async throws {
    let apple = CountingBackend(name: "Apple")
    let mlx = CountingBackend(name: "MLX")
    let said = try await OnDeviceTextEngine(backends: [apple, mlx]).respond(prompt: "Hello", maxTokens: 16)
    #expect(said == "answer")
    #expect(apple.calls == 1 && mlx.calls == 0)
}

// MARK: the context limit, refused before the model is called

@Test func aPromptOverTheLimitIsRefusedBeforeTheCallWithTheSentenceNamingTheLimit() async {
    let backend = CountingBackend()
    let engine = OnDeviceTextEngine(backends: [backend])
    let prompt = String(repeating: "x", count: Int(Double(OnDeviceLimits.contextTokens) * OnDeviceLimits.charsPerTokenFloor) + 500)
    do { _ = try await engine.respond(prompt: prompt, maxTokens: 160); Issue.record("an over-long prompt reached the model") }
    catch let e as LocalModelError {
        guard case .promptTooLong(_, let limit) = e else { Issue.record("wrong error \(e)"); return }
        #expect(limit == OnDeviceLimits.contextTokens)
        #expect(e.description.contains(String(OnDeviceLimits.contextTokens)), "the sentence must name the limit")
    } catch { Issue.record("wrong error \(error)") }
    #expect(backend.calls == 0, "the model was called")
}

@Test func theLimitCountsTheAnswerAsWellAsThePrompt() async throws {
    // The prompt alone fits; prompt plus the answer budget does not.
    let backend = CountingBackend()
    let engine = OnDeviceTextEngine(backends: [backend])
    let maxTokens = 160
    let fitChars = Int(Double(OnDeviceLimits.contextTokens - maxTokens) * OnDeviceLimits.charsPerTokenFloor)
    _ = try await engine.respond(prompt: String(repeating: "x", count: fitChars), maxTokens: maxTokens)
    #expect(backend.calls == 1, "a prompt that exactly fits was refused")
    do { _ = try await engine.respond(prompt: String(repeating: "x", count: fitChars + 10), maxTokens: maxTokens); Issue.record("one token too many was allowed") }
    catch is LocalModelError {}
    #expect(backend.calls == 1, "the over-limit call reached the model")
}

@Test func anOverLimitPromptOnAnUnavailableMacSaysUnavailableNotTooLong() async {
    let engine = OnDeviceTextEngine(backends: [ArmedAppleBackend(reason: .deviceNotEligible)])
    let prompt = String(repeating: "x", count: OnDeviceLimits.contextTokens * 4)
    do { _ = try await engine.respond(prompt: prompt, maxTokens: 160); Issue.record("answered") }
    catch let e as LocalModelError { if case .unavailable = e {} else { Issue.record("wrong error \(e)") } }
    catch { Issue.record("wrong error \(error)") }
}

// MARK: the row Settings shows

@Test func theRowSaysReadyWhenABackendIsReady() {
    let st = OnDeviceStatus.make(backends: [ArmedAppleBackend(reason: nil)], install: .notInstalled)
    #expect(st.kind == .ready)
    #expect(st.backend == ArmedAppleBackend(reason: nil).name)
    #expect(st.row["state"] as? String == "ready" && st.row["reason"] as? String == "")
}

@Test func theRowSaysUnavailableWithTheReasonForEachAppleReason() {
    for reason in AppleReason.allCases {
        let st = OnDeviceStatus.make(backends: [ArmedAppleBackend(reason: reason)], install: .notInstalled)
        #expect(st.kind == .unavailable)
        #expect(st.words.contains(reason.words))
        #expect(st.row["state"] as? String == "unavailable")
        #expect(st.row["reason"] as? String == st.words, "the row's reason is the sentence the pane shows")
    }
}

@Test func theRowShowsDownloadProgressWhileTheModelDownloads() {
    let off = CountingBackend(ready: .unavailable(reason: "Not installed."))
    let half = OnDeviceStatus.make(backends: [off], install: .downloading(received: 50, total: 100))
    #expect(half.kind == .downloading && half.percent == 50)
    #expect(half.row["percent"] as? Int == 50 && half.row["state"] as? String == "downloading")
    #expect(!half.words.isEmpty)
    let checking = OnDeviceStatus.make(backends: [off], install: .verifying)
    #expect(checking.kind == .downloading && checking.percent == 100)
    let start = OnDeviceStatus.make(backends: [off], install: .downloading(received: 0, total: 100))
    #expect(start.percent == 0)
}

@Test func thePausedAndFailedStatesReadAsUnavailableWithWhy() {
    let off = CountingBackend(ready: .unavailable(reason: "Not installed."))
    #expect(OnDeviceStatus.make(backends: [off], install: .paused(received: 10, total: 100)).kind == .unavailable)
    let failed = OnDeviceStatus.make(backends: [ArmedAppleBackend(reason: .notEnabled)], install: .failed(reason: "The server answered 404."))
    #expect(failed.kind == .unavailable)
    #expect(failed.words.contains(AppleReason.notEnabled.words))
}

@Test func aReadyBackendOutranksADownloadInProgress() {
    let st = OnDeviceStatus.make(backends: [ArmedAppleBackend(reason: nil), CountingBackend(ready: .unavailable(reason: "No."))],
                                 install: .downloading(received: 10, total: 100))
    #expect(st.kind == .ready && st.percent == nil)
}

@Test func theRigArmsAppleFromItsKeyAndUnknownKeysAreOther() {
    #expect(ArmedAppleBackend.armed("ready").availability().isReady)
    #expect(ArmedAppleBackend.armed("not_enabled").reason == .notEnabled)
    #expect(ArmedAppleBackend.armed("nonsense").reason == .other)
}

// MARK: speed and memory: loads on first use, unloads when idle

@Test func theModelLoadsOnFirstUseAndNotBefore() async throws {
    let clock = Clock()
    let r = ModelResidency<Int>(idleSeconds: 300, now: { clock.now }, autoSweep: false)
    #expect(await r.isLoaded == false, "something loaded at launch")
    let v = try await r.use(load: { 7 }) { $0 * 2 }
    #expect(v == 14)
    #expect(await r.isLoaded)
    #expect(await r.holder.loadCount == 1)
}

@Test func theModelUnloadsAfterTheIdleTimeFromTheLedgerAndLoadsAgainOnTheNextUse() async throws {
    let clock = Clock()
    let r = ModelResidency<Int>(now: { clock.now }, autoSweep: false)   // default idle time: WhooshConstants.EMBED_IDLE_UNLOAD_S
    _ = try await r.use(load: { 1 }) { $0 }
    clock.now += TimeInterval(WhooshConstants.EMBED_IDLE_UNLOAD_S) - 1
    #expect(await r.unloadIfIdle() == false, "unloaded before the idle time")
    #expect(await r.isLoaded)
    clock.now += 1
    #expect(await r.unloadIfIdle(), "still loaded after the idle time")
    #expect(await r.isLoaded == false)
    _ = try await r.use(load: { 2 }) { $0 }
    let loads = await r.holder.loadCount, unloads = await r.holder.unloadCount
    #expect(loads == 2 && unloads == 1)
}

@Test func aUseKeepsTheModelFromBeingTakenForIdle() async throws {
    let clock = Clock()
    let r = ModelResidency<Int>(idleSeconds: 300, now: { clock.now }, autoSweep: false)
    _ = try await r.use(load: { 1 }) { $0 }
    clock.now += 200
    _ = try await r.use(load: { 1 }) { $0 }       // used again at +200
    clock.now += 200                               // 400 since the load, 200 since the last use
    #expect(await r.unloadIfIdle() == false)
    #expect(await r.holder.loadCount == 1, "a second use reloaded the model")
}

@Test func unloadingClearsTheMemoryHook() async throws {
    let clock = Clock()
    final class Flag: @unchecked Sendable { var cleared = false }
    let flag = Flag()
    let r = ModelResidency<Int>(idleSeconds: 10, now: { clock.now }, autoSweep: false, onUnload: { flag.cleared = true })
    _ = try await r.use(load: { 1 }) { $0 }
    clock.now += 10
    _ = await r.unloadIfIdle()
    #expect(flag.cleared)
}
