import Foundation
import Testing
@testable import WhooshUI

// pieces (SPEC A151): the model's input cap stays as a safety and counts each input it shortens, so a check can assert 0.
// `InputCut` is what GemmaEmbedder.run applies to every batch (GemmaEmbedder.cutInputs reads its count); no model is loaded here.

@Suite struct PiecesInputCutTests {
    /// An input over the cap is cut to the cap and counted; one within it is untouched and not counted.
    /// Broken copy (red): in `InputCut.apply`, `let over = ids.filter { $0.count > limit }.count` -> `let over = 0`.
    @Test func anInputOverTheCapIsCutAndCounted() {
        let cut = InputCut()
        let out = cut.apply([[1, 2, 3], Array(0 ..< 600)], limit: 512)
        #expect(out[0] == [1, 2, 3])
        #expect(out[1].count == 512)
        #expect(out[1] == Array(0 ..< 512))
        #expect(cut.cutCount == 1)
    }

    /// Inputs at or under the cap leave the count at 0 (what the lead's check asserts on the owner's mail), across calls.
    /// Broken copy (red): `ids.filter { $0.count > limit }` -> `ids.filter { $0.count >= limit }`.
    @Test func inputsAtOrUnderTheCapAreNotCounted() {
        let cut = InputCut()
        _ = cut.apply([Array(0 ..< 512), [7]], limit: 512)
        _ = cut.apply([Array(0 ..< 100)], limit: 512)
        #expect(cut.cutCount == 0)
    }

    /// The cut is kept as a safety: a long input never reaches the model whole.
    /// Broken copy (red): `return ids.map { Array($0.prefix(limit)) }` -> `return ids`.
    @Test func theCutStaysAsASafety() {
        let cut = InputCut()
        let out = cut.apply([Array(0 ..< 2_000)], limit: 512)
        #expect(out.allSatisfy { $0.count <= 512 })
        #expect(cut.cutCount == 1)
    }
}
