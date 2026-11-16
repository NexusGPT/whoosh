import Foundation
#if canImport(Tokenizers)
import Tokenizers

/// The checkpoint's own tokenizer.json (ModernBERT byte-level BPE) through huggingface/swift-transformers (Apache-2.0).
public struct LayaTokenizer: LayaTokenizing {
    let inner: any Tokenizer
    public let clsId: Int, sepId: Int, maskId: Int
    public let maskText = "[MASK]"

    /// `folder` holds tokenizer.json and tokenizer_config.json.
    public init(folder: URL) async throws {
        inner = try await AutoTokenizer.from(modelFolder: folder)
        guard let c = inner.convertTokenToId("[CLS]"), let s = inner.convertTokenToId("[SEP]"), let m = inner.convertTokenToId("[MASK]")
        else { throw LocalModelError.failed("The decision model's tokenizer has no [CLS], [SEP] or [MASK] token. Delete the model and download it again.") }
        clsId = c; sepId = s; maskId = m
    }
    public func encode(_ text: String) -> [Int] { inner.encode(text: text, addSpecialTokens: false) }
}
#endif
