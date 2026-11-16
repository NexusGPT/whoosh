import Foundation

public protocol LayaTokenizing {
    /// token ids for text, no special tokens added
    func encode(_ text: String) -> [Int]
    var clsId: Int { get }
    var sepId: Int { get }
    var maskId: Int { get }
    var maskText: String { get }
}

/// Port of rl_common.py build_sequence (Apache-2.0, convaiinnovations/laya):
/// [CLS] <type> question: instructions [SEP] [MASK] opt0 [MASK] opt1 ... [SEP] state [SEP]
public enum LayaSequence {
    /// Lane fix-aisort round 5: the token count of each part, before and after its cut (the rig's trace, `LayaTrace`).
    public struct Parts: Sendable {
        public var headFull = 0, headKept = 0, optionsFull: [Int] = [], optionsKept: [Int] = [], stateFull = 0, stateKept = 0, total = 0
    }

    public static func build(tokenizer tok: some LayaTokenizing, state: String, question q: LayaQuestion,
                             maxLen: Int, headMaxLen: Int) -> (ids: [Int], markers: [Int]) {
        let t = buildTraced(tokenizer: tok, state: state, question: q, maxLen: maxLen, headMaxLen: headMaxLen)
        return (t.ids, t.markers)
    }

    /// `build`, with the size of each part.
    public static func buildTraced(tokenizer tok: some LayaTokenizing, state: String, question q: LayaQuestion,
                                   maxLen: Int, headMaxLen: Int) -> (ids: [Int], markers: [Int], parts: Parts) {
        var parts = Parts()
        let opts = q.renderedOptions()
        let ins = q.instructions.replacingOccurrences(of: tok.maskText, with: " ")
        var head = tok.encode("\(q.kind.rawValue) question: \(ins)")
        parts.headFull = head.count
        parts.optionsFull = opts.map { tok.encode(" " + $0.replacingOccurrences(of: tok.maskText, with: " ")).count + 1 }
        var optIds: [[Int]] = opts.map { o in
            [tok.maskId] + Array(tok.encode(" " + o.replacingOccurrences(of: tok.maskText, with: " ")).prefix(OnDeviceLimits.layaOptionTokenCap))
        }
        var budget = headMaxLen - optIds.reduce(0) { $0 + $1.count }
        if budget < OnDeviceLimits.layaOptionBudgetFloor {
            let per = max(4, (headMaxLen - OnDeviceLimits.layaOptionBudgetFloor) / max(1, optIds.count))
            optIds = optIds.map { Array($0.prefix(per)) }
            budget = headMaxLen - optIds.reduce(0) { $0 + $1.count }
        }
        head = Array(head.prefix(max(OnDeviceLimits.layaMinHeadTokens, budget)))
        var ids = [tok.clsId] + head + [tok.sepId]
        var markers: [Int] = []
        for o in optIds { markers.append(ids.count); ids += o }
        ids.append(tok.sepId)
        let room = max(0, maxLen - ids.count - 1)
        let stFull = tok.encode(state.replacingOccurrences(of: tok.maskText, with: " "))
        let st = Array(stFull.prefix(room))
        ids += st + [tok.sepId]
        parts.headKept = head.count; parts.optionsKept = optIds.map(\.count); parts.stateFull = stFull.count; parts.stateKept = st.count
        parts.total = min(ids.count, maxLen)
        return (Array(ids.prefix(maxLen)), markers.filter { $0 < maxLen }, parts)
    }
}
