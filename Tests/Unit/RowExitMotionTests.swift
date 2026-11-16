import AppKit
import Foundation
import Testing
@testable import WhooshUI

// The mark-done exit motion (owner 2026-11-11: a leaving mail slides left and dissolves, gently and fast). Pure parts only:
// no window is opened. Each test names, in its trailing comment, why it fails on the code before this lane.
// All of them name RowExitEffect / WhooshMotion.rowExit*, which the old code does not have (it then fails to build, the
// honest red), except the one on RowCollapse.run, which builds on the old code and returns false there.

private let full: CGFloat = 44

@Test func theLeavingRowIsUntouchedAtProgressZero() {
    let v = RowExitEffect.values(elapsedMs: 0, full: full)
    #expect(v.offsetX == 0 && v.opacity == 1 && v.height == full)   // old: no RowExitEffect
}

@Test func atTheDelayTheContentHasMovedLeftAndTheGapHasNotStartedToClose() {
    let v = RowExitEffect.values(elapsedMs: Double(WhooshMotion.rowExitCloseDelayMs), full: full)
    #expect(v.offsetX < 0 && v.offsetX > -WhooshMotion.rowExitSlidePt)
    #expect(v.opacity < 1 && v.opacity > 0)
    #expect(v.height == full)   // old: the height closed from the first frame, nothing slid
}

@Test func atTheEndTheRowIsGoneLeftFadedAndClosed() {
    let v = RowExitEffect.values(elapsedMs: Double(WhooshMotion.rowExitTotalMs), full: full)
    #expect(v.offsetX == -WhooshMotion.rowExitSlidePt)
    #expect(v.opacity == 0)
    #expect(v.height == 0)   // old: no offset
}

@Test func theFourNumbersDriveTheEffect() {
    let v = RowExitEffect.values(elapsedMs: 1000, full: full, slidePt: 56, slideMs: 170, delayMs: 60, closeMs: 140)
    #expect(v.offsetX == -56 && v.opacity == 0 && v.height == 0)
    let w = RowExitEffect.values(elapsedMs: 10, full: full, slidePt: 20, slideMs: 90, delayMs: 20, closeMs: 90)
    #expect(w.height == full && w.offsetX < 0 && w.offsetX > -20)
}

@Test func aSlideDistanceOfZeroMeansTheRowsOwnWidth() {
    let v = RowExitEffect.values(elapsedMs: 1000, full: full, slidePt: 0, rowWidth: 420)
    #expect(v.offsetX == -420)   // old: no distance parameter
}

@Test func theOpacityNeverRisesAndEndsAtZero() {
    var lastO = 1.0
    for t in stride(from: 0.0, through: Double(WhooshMotion.rowExitTotalMs), by: 5) {
        let v = RowExitEffect.values(elapsedMs: t, full: full)
        #expect(v.opacity <= lastO + 1e-9)
        lastO = v.opacity
    }
    #expect(abs(RowExitEffect.values(elapsedMs: 1000, full: full).opacity) < 1e-9)   // old: the fade ran 1 to 0 in one piece
}

@Test func theGapClosesMonotonicallyAndTheSlideNeverReturns() {
    var lastH = full, lastX: CGFloat = 0
    for t in stride(from: 0.0, through: Double(WhooshMotion.rowExitTotalMs), by: 5) {
        let v = RowExitEffect.values(elapsedMs: t, full: full)
        #expect(v.height <= lastH && v.offsetX <= lastX)
        lastH = v.height; lastX = v.offsetX
    }
}

@Test func reducedMotionGivesNoSlide() {
    let v = RowExitEffect.values(elapsedMs: 60, full: full, reduceMotion: true)
    #expect(v.offsetX == 0)   // old: no slide at all, no parameter to read
}

@Test func theWholeExitTakesNoMoreThanTheMotionCeiling() {
    #expect(WhooshMotion.rowExitTotalMs <= WhooshConstants.MOTION_MAX_MS)   // old: no rowExitTotalMs
    #expect(WhooshMotion.rowExitTotalMs == max(WhooshMotion.rowExitSlideMs, WhooshMotion.rowExitCloseDelayMs + WhooshMotion.rowExitCloseMs))
}

@MainActor @Test func aSecondRemovalThirtyMillisecondsAfterTheFirstIsAnimated() throws {
    try #require(!NSWorkspace.shared.accessibilityDisplayShouldReduceMotion, "Reduced Motion is on: nothing glides")
    _ = RowCollapse.run { true }
    _ = RowCollapse.run { true }   // back to back: the gap is a few ms, far under 30
    #expect(RowCollapse.lastGapMs.map { $0 < 30 } ?? false)
    #expect(RowCollapse.lastGlided)   // old: false, the rapid rule jumped a second removal inside FOCUS_SLIDE_MS (80 ms)
}
