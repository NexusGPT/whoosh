import Foundation

/// The only file of this folder that names the generated functions (Sources/CoreBridge/Generated/engine.swift, made by
/// scripts/gen-swift.sh from core/crates/engine/src/crawl.rs: `set_background_conditions`, `note_owner_activity`,
/// `set_background_embedder`, `expect_background_embedder`). The names below are uniffi's camel-case of the Rust ones; they exist after the lead regenerates.
/// The functions reach every live core of the process, and a core that opens later gets the last values (sync-full-r1 note).
public struct LiveBackgroundEngine: BackgroundEngine {
    public init() {}

    public func setConditions(batteryPercent: Int, charging: Bool, thermalSerious: Bool) {
        setBackgroundConditions(batteryPercent: UInt8(clamping: batteryPercent), charging: charging, thermalSerious: thermalSerious)
    }

    public func ownerActed() { noteOwnerActivity() }

    public func setEmbedder(_ embedder: (any Embedder)?) {
        // pieces (crawl.rs `set_embedding_model_folder`, generated after the lead regenerates): the model's folder before the model
        setEmbeddingModelFolder(path: (embedder as? ModelFolderNaming)?.modelFolder)
        setBackgroundEmbedder(embedder: embedder)
    }

    /// model-late: `expect_background_embedder` (crawl.rs), a new generated function: it exists after the lead regenerates the bindings.
    public func expectEmbedder() { expectBackgroundEmbedder() }
}
