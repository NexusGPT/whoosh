import Foundation

/// The Superhuman keys beyond E H P L: delete (⇧#), junk (⇧!), star (S), mute and unmute (⇧M). Each is a core Command through
/// AppModel.perform on the targets (the selection, else the focused row). Junk and star write no undo entry in
/// the contract (deltas 8): the capsule says so and has no Undo button. A delete does: Z restores it.
@MainActor
enum TriageKeys {
    static var state: InboxListState { .shared }

    /// ⇧# (the live id `delete`): delete the targets one by one; the first refusal stops the run, the refused row stays and the line names why.
    static func trash(_ m: AppModel) -> Bool {
        let ids = TriageActions.targets(m)
        guard !ids.isEmpty else { return false }
        var done = 0
        for id in ids {
            guard RowCollapse.run({ m.perform(.deleteMessage(messageId: id)) }) else { break }
            done += 1
        }
        guard done > 0 else { return false }
        state.clearSelection()
        // the core keeps an undo entry per delete (the fixture does; the real core's UndoAction has DeleteMessage): Z brings it back
        state.say(done == 1 ? Said("Deleted") : Said("Deleted %lld", done))
        return done == ids.count
    }

    /// ⇧! (the live id `junk`): the targets go to spam in one command.
    static func spam(_ m: AppModel) -> Bool {
        let ids = TriageActions.targets(m)
        guard !ids.isEmpty, RowCollapse.run({ m.perform(.markSpam(messageIds: ids)) }) else { return false }
        state.clearSelection()
        state.say(ids.count == 1 ? Said("Moved to junk · no undo for junk yet") : Said("Moved %lld to junk · no undo for junk yet", ids.count), undoable: false)
        return true
    }

    /// The palette command "Not spam" (no key): the targets return to the inbox. Core `MarkNotSpam` (delta 19), no undo entry; the
    /// spam filter learns them as not junk. Acts on the selection, else the focused row (on the Junk view, where the rows are).
    static func notSpam(_ m: AppModel) -> Bool {
        let ids = TriageActions.targets(m)
        guard !ids.isEmpty, RowCollapse.run({ m.perform(.markNotSpam(messageIds: ids)) }) else { return false }
        state.clearSelection()
        state.say(ids.count == 1 ? Said("Back in the list") : Said("%lld back in the list", ids.count), undoable: false)
        return true
    }

    /// S: stars the targets, or unstars them when every one is starred already.
    static func star(_ m: AppModel) -> Bool {
        let ids = TriageActions.targets(m)
        guard !ids.isEmpty else { return false }
        let on = !ids.allSatisfy { state.starred.contains($0) }
        guard m.perform(.star(messageIds: ids, starred: on)) else { return false }
        ListFilters.refreshStarred(m)
        state.say(on ? Said("Starred") : Said("Unstarred"), undoable: false)
        return true
    }

    /// ⇧M toggles (the owner's key set has one mute key): it mutes the targets' threads, which leave the inbox, or unmutes them when the
    /// core says every target is muted already (the Muted view, All mail): the thread returns to the inbox and leaves the Muted view.
    static func mute(_ m: AppModel) -> Bool {
        let ids = TriageActions.targets(m)
        guard !ids.isEmpty else { return false }
        let unmute = ids.allSatisfy { m.core.state(path: "message/\($0)/muted") == "true" }
        guard m.perform(.mute(messageIds: ids, muted: !unmute)) else { return false }
        state.clearSelection()
        state.say(unmute ? Said("Unmuted") : Said("Muted"))   // the engine keeps one undo entry per mute or unmute (delta 19): Z puts each message's earlier flag back
        return true
    }

    /// ⇧E mark not done (the one owner since reader r7): brings back the archived thread on screen, never a remembered one.
    /// On the Archive view (G then E) it acts on the selection or the focused thread; elsewhere on the focused thread only
    /// when the core says it is archived; otherwise it passes on with the words. Core `unarchive` (delta 11), no undo entry.
    static func markNotDone(_ m: AppModel) -> Bool {
        let ids: [String]
        if m.mailbox == .archive {
            ids = TriageActions.targets(m)
        } else {
            ids = m.shownMessageId.map { [$0] }?.filter { m.core.state(path: "message/\($0)/archived") == "true" } ?? []
        }
        guard !ids.isEmpty else {
            m.lastFailure = ActionFailed(accountName: "Whoosh", reasonWords: Words.text("Nothing archived to bring back"), retryable: false)
            return false
        }
        guard RowCollapse.run({ m.perform(.unarchive(messageIds: ids)) }) else { return false }
        state.clearSelection()
        state.say(ids.count == 1 ? Said("Back in the list") : Said("%lld back in the list", ids.count), undoable: false)
        return true
    }
}
