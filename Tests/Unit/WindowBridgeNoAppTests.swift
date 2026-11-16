import Foundation
import Testing
@testable import WhooshUI
#if os(macOS)
import AppKit
#endif

// Round main76: the unit run trapped at WindowBridge.swift:43 (`NSApp.windows` with NSApp nil) in a test process and printed no summary.
// A process with no NSApplication has no windows: the socket's window facts answer empty, never trap.
// Mutant: read `NSApp.windows` directly in WindowBridge.frames() or window(for:); this test then dies with SIGTRAP.
// A `swift test` process has no NSApplication unless something made one, so `NSApp` is nil here and this test runs the nil path; were an application to exist
// (no visible window in a test process either way) every line below still holds.
@MainActor @Test func windowFactsAreEmptyWhenThereIsNoApplication() {
    #if os(macOS)
    #expect(WindowBridge.frames().isEmpty)
    #expect(WindowBridge.window(for: .macMain) == nil)
    #expect(WindowBridge.mainWindow() == nil)
    #expect(!WindowBridge.isShowing(.macMain))
    #endif
}
