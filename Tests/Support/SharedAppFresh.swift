import Foundation
import Testing
@testable import WhooshUI

/// The unit tests that read the app's screens share ONE `AppModel.shared` and ONE fixture core (`CoreHost.shared`) for the whole test process.
/// Round main76 and the first lane runs showed what that does: a test that reads the list got a row another test had filed, a test that needs an open
/// thread passed or failed by what ran before it (ReaderTests, QuickReplyTests, NotchTests: "no conversation is open", `m.rows.first` nil, unread 0).
/// `SharedApp.begin()` hands the shared model back in its launch state ONCE PER TEST (the first call a test makes; later calls inside the same test change nothing,
/// so a test can set up, then read through its own helpers): the fixture core back to its seed (`restoreSeed`, FixtureReset.swift), the view state back to
/// launch (`FixtureEvents.viewState(reset: true)`, the part of the socket's `fixture reset` that does not touch AppKit: that verb's `reset` closes windows
/// through `NSApp`, which a test process does not have), typed text, overlay and the failure line cleared, and any open thread or quick reply closed with Escape.
@MainActor
enum SharedApp {
    private static var lastTest: Test.ID?

    static func begin() -> AppModel {
        let m = AppModel.shared
        let id = Test.current?.id
        guard m.host.kind == .fixture, id == nil || id != lastTest else { return m }
        lastTest = id
        CoreHost.shared.fixture?.restoreSeed()
        FixtureEvents.viewState(m, reset: true)
        m.typed = ""; m.overlay = nil; m.surface = SurfaceSlot.macMain.id; m.lastFailure = nil
        _ = m.press("Escape"); _ = m.press("Escape")   // an open thread, quick reply or palette from an earlier test
        m.typed = ""; m.overlay = nil; m.surface = SurfaceSlot.macMain.id
        m.refresh()
        return m
    }
}
