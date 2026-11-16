import Foundation
import Testing
@testable import WhooshUI

// Lane query-embed: the error the on-device model throws to the engine names the step that failed and keeps the library's own words,
// so the engine's log (`search: the model did not embed the query: <words>`) says why. No model, no MLX: the words alone.
// Red-first broken copies (one line each, in Sources/Features/Embedding/EmbedStepFailure.swift):
// - stepNamedWithTheLibrarysWords: `case let failure as EmbedStepFailure:` returning `.Unexpected(reasonWords: "embedding failed")`
// - nsErrorKeepsItsDomainAndCode: the default arm using `error.localizedDescription` (the base's words) instead of `String(describing:)`
// - badVectorIsTheCheckStep: the `badVector` arm removed (it then falls to "embedding failed: badVector(...)")
// - missingModelKeepsItsOwnWords: the `weightsMissing` arm removed
// - folderFactsCountTheWeightFilesAndFollowALink: `folderFacts` counting `listed.count` for the weight files

private func words(_ e: CoreError) -> String {
    if case .Unexpected(let w) = e { return w }
    return "not Unexpected: \(e)"
}

@Test func stepNamedWithTheLibrarysWords() {
    for step in EmbedStep.allCases {
        let w = words(EmbedStepFailure.coreError(EmbedStepFailure(step: step, detail: "keyNotFound(model.embed_tokens.weight)")))
        #expect(w == "the search model could not \(step.rawValue): keyNotFound(model.embed_tokens.weight)")
    }
    #expect(EmbedStep.allCases.map(\.rawValue) == ["load", "tokenise", "run", "check"])
}

@Test func nsErrorKeepsItsDomainAndCode() {
    let e = NSError(domain: "MLXEmbedders.Load", code: 7, userInfo: [NSLocalizedFailureReasonErrorKey: "unsupported model type"])
    let w = words(EmbedStepFailure.coreError(e))
    #expect(w.hasPrefix("embedding failed: "))
    #expect(w.contains("MLXEmbedders.Load"))
    #expect(w.contains("7"))
}

@Test func badVectorIsTheCheckStep() {
    let w = words(EmbedStepFailure.coreError(GemmaEmbeddingError.badVector(reasonWords: "the model gave a vector of zeros")))
    #expect(w == "the search model could not check: the model gave a vector of zeros")
}

@Test func missingModelKeepsItsOwnWords() {
    let w = words(EmbedStepFailure.coreError(GemmaEmbeddingError.weightsMissing(reasonWords: "The search model is not installed yet. Install it from Settings.")))
    #expect(w == "The search model is not installed yet. Install it from Settings.")
    #expect(words(EmbedStepFailure.coreError(nil)) == "embedding gave no result")
}

@Test func folderFactsCountTheWeightFilesAndFollowALink() throws {
    let fm = FileManager.default
    let root = fm.temporaryDirectory.appendingPathComponent("embed-step-\(UUID().uuidString)", isDirectory: true)
    let real = root.appendingPathComponent("revision", isDirectory: true)
    try fm.createDirectory(at: real, withIntermediateDirectories: true)
    defer { try? fm.removeItem(at: root) }
    for name in ["config.json", "model.safetensors", "tokenizer.json"] { try Data("x".utf8).write(to: real.appendingPathComponent(name)) }
    let link = root.appendingPathComponent("link")
    try fm.createSymbolicLink(at: link, withDestinationURL: real)
    let direct = EmbedStepFailure.folderFacts(real)
    #expect(direct.contains("3 entries listed, 1 weight files listed"), "\(direct)")
    let viaLink = EmbedStepFailure.folderFacts(link)
    #expect(viaLink.contains("folder \(link.path)"), "\(viaLink)")
    #expect(viaLink.contains("resolves to \(real.resolvingSymlinksInPath().path)"), "\(viaLink)")
}
