import Foundation
import Observation

/// The bodies the shown thread already read. A key (N, P, O, ⇧O, Tab, ⌘O) changes the reader's own state and the pane
/// draws again; without this every such draw read the open body from the core again, and every expanded earlier message
/// its text. Round 8 measured the double read on a click; this closes the same hole on the keys. One thread's worth:
/// cleared when another message is shown or the reader opens afresh, never held across threads (RAM). A message the
/// core holds no body for is remembered as empty too (round 11): the first redraws read it again each time (main51: a
/// header toggle read 2 bodies, keys 4, all for earlier messages the fixture holds no body for). A body that arrives
/// later is read at once: the core announces its thread and the spine calls `forget(messageIds:)` (body-r1).
@MainActor @Observable
final class ReaderBodies {
    static let shared = ReaderBodies()
    @ObservationIgnored private var held: [String: MessageBody] = [:]
    @ObservationIgnored private var empty: Set<String> = []
    /// Bumped when a body lands after the preview showed (`forget`): the card that drew the preview reads again and swaps in place (A129).
    private(set) var revision = 0
    /// The body view's content built from a held body (layout, picture counts and plain text are regex passes): built once per message, so a redraw of the pane costs none.
    @ObservationIgnored private var built: [String: MessageBodyContent] = [:]

    func content(_ m: AppModel, _ id: String, make: (MessageBody) -> MessageBodyContent) -> MessageBodyContent? {
        if let c = built[id] { return c }
        guard let b = body(m, id) else { return nil }
        let c = make(b)
        built[id] = c
        return c
    }

    func body(_ m: AppModel, _ id: String) -> MessageBody? {
        if let b = held[id] { return b }
        if empty.contains(id) { return nil }
        ReaderPerf.shared.noteBodyRead()
        guard let b = m.core.body(messageId: id) else { empty.insert(id); return nil }
        held[id] = b
        return b
    }

    func clear() { held = [:]; empty = []; built = [:] }

    /// body-r1: the core downloaded these messages' bodies after the reader showed their preview (`threadChanged`): the held
    /// copies and the misses are dropped, so the next draw reads the stored body.
    func forget(messageIds: [String]) {
        for id in messageIds { held[id] = nil; empty.remove(id); built[id] = nil }
        revision += 1
    }
}
