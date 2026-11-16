import Foundation
import Observation

/// The install step's state: one model installer, one engine, what the surface shows. Input changes state on the frame it lands.
@MainActor @Observable
final class OnDeviceSetup {
    static let shared = OnDeviceSetup()
    enum Reply: Equatable { case none, thinking, answer(String), refused(String) }

    private(set) var state: ModelInstallState
    private(set) var reply = Reply.none
    private(set) var spec: MlxModelSpec
    private var installer: ModelInstaller
    private var backend: MlxBackend
    private var apple: any TextBackend
    private var appleKey: String?
    private var task: Task<Void, Never>?

    private init() {
        let rig = RigFixture.load()
        let sp = rig?.spec ?? .qwen25
        let inst = ModelInstaller(spec: sp, directory: rig?.directory ?? ModelInstaller.defaultDirectory())
        spec = sp; installer = inst; backend = MlxBackend(installer: inst); state = inst.state
        if let r = rig { apple = r.apple } else { apple = AppleIntelligenceBackend() }
        appleKey = rig?.appleKey
        wire()
    }

    /// The Settings row, the Try-it field and every AI feature read this one engine, so "unavailable" and the context
    /// limit are said the same everywhere. Apple Intelligence answers first where it is ready; the downloaded model is next.
    var engine: OnDeviceTextEngine { OnDeviceTextEngine(backends: [apple, backend]) }
    var availabilityWords: String { engine.availability().words }
    /// The state the Settings provider row shows (ready, downloading with progress, unavailable with the reason).
    var status: OnDeviceStatus { OnDeviceStatus.make(backends: [apple, backend], install: state) }
    /// The LocalModel the core calls, on the same engine (never a cloud provider: no backend here talks to one).
    var localModel: OnDeviceLocalModel { OnDeviceLocalModel(engine: engine) }

    private func wire() {
        installer.onChange = { s in Task { @MainActor in OnDeviceSetup.shared.state = s } }
    }

    /// The rig's fixture server replaces the pinned model when the check writes its pointer file (never on the owner's copy).
    func reloadRigFixture() {
        guard let rig = RigFixture.load(),
              rig.spec != spec || rig.directory != installer.directory || rig.appleKey != appleKey else { return }
        task?.cancel()
        apple = rig.apple; appleKey = rig.appleKey
        spec = rig.spec
        installer = ModelInstaller(spec: spec, directory: rig.directory)
        backend = MlxBackend(installer: installer)
        state = installer.state; reply = .none
        wire()
    }

    func install() {
        reloadRigFixture()
        guard !state.isBusy else { return }
        let i = installer
        task = Task { _ = await i.start() }
    }
    func pause() { installer.pause() }
    func cancel() { installer.cancel() }
    func refresh() { reloadRigFixture(); installer.refresh() }

    /// The question is the shell's `typed` buffer, so the socket's `type` verb and the field are one source.
    @discardableResult
    func ask(_ m: AppModel) -> Bool {
        let prompt = m.typed.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !prompt.isEmpty, reply != .thinking else { return false }
        reply = .thinking
        m.typed = ""
        let e = engine
        task = Task {
            do { reply = .answer(try await e.respond(prompt: prompt, maxTokens: 160)) }
            catch let err as LocalModelError { reply = .refused(err.description) }
            catch { reply = .refused(error.localizedDescription) }
            installer.onChange?(installer.state)
        }
        return true
    }
}
