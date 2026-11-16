import Foundation
import Testing
@testable import WhooshUI

// Fault 1 (app-faults-2): `shot <screen>` named the window by SwiftUI's own identifier (`surface-AppWindow-1`) read before the app tagged
// it, so a later `shot <name> window:surface-AppWindow-1` was refused. Every socket answer now names a window through one rule.
// These tests run the pure rule: no window is opened, no NSApplication is made.
// Mutant: make `windowId` return the identifier first; the first test then answers `surface-AppWindow-1` and fails.

@MainActor @Test func aSurfaceWindowIsNamedByItsSurfaceNotBySwiftUIsName() {
    #expect(WindowBridge.windowId(surface: "mac-search", identifier: "surface-AppWindow-1") == "mac-search")
}

@MainActor @Test func aWindowWithNoSurfaceIsNamedByItsIdentifier() {
    #expect(WindowBridge.windowId(surface: nil, identifier: "mac-main-AppWindow-1") == "mac-main-AppWindow-1")
}

@MainActor @Test func aWindowWithNeitherNameIsNamedWithTheEmptyString() {
    #expect(WindowBridge.windowId(surface: nil, identifier: nil) == "")
    #expect(WindowBridge.windowId(surface: "", identifier: nil) == "")
}

@MainActor @Test func anEmptySurfaceFallsBackToTheIdentifier() {
    #expect(WindowBridge.windowId(surface: "", identifier: "mac-settings") == "mac-settings")
}

// Fault 2 (app-faults-2): `labels` answered `text: null` for a Text styled with `.whooshText`, because the modifiers that style adds were not
// in `FrameProbe.leafModifiers`. The decision is one pure function on the printed modifier type name. The walk itself (`leafText`) reads a real
// view and claims copies, so it is exercised by the app's `labels` verb, not here.
// Mutant: drop `leafModifierFamilies` from `isLeafModifier`; the family tests then fail.

@MainActor @Test func theTypesTheAppsOwnTextStyleAddsAreLeafModifiers() {
    #expect(FrameProbe.isLeafModifier("_EnvironmentKeyWritingModifier<Optional<Font>>"))
    #expect(FrameProbe.isLeafModifier("_EnvironmentKeyWritingModifier<Optional<CGFloat>>"))
    #expect(FrameProbe.isLeafModifier("_EnvironmentKeyTransformModifier<Optional<Font>>"))
    #expect(FrameProbe.isLeafModifier("_PaddingLayout"))
    #expect(FrameProbe.isLeafModifier("_ForegroundStyleModifier<ThemeColor>"))
    #expect(FrameProbe.isLeafModifier("_TraitWritingModifier<TransitionTraitKey>"))
}

@MainActor @Test func aModifierThatCarriesAViewOrAnActionIsNotALeafModifier() {
    #expect(!FrameProbe.isLeafModifier("_OverlayModifier<Text>"))
    #expect(!FrameProbe.isLeafModifier("_BackgroundModifier<Color>"))
    #expect(!FrameProbe.isLeafModifier("AddGestureModifier<TapGesture>"))
    #expect(!FrameProbe.isLeafModifier("_TaskModifier"))
    #expect(!FrameProbe.isLeafModifier("_EnvironmentKeyWritingModifierX"))
    #expect(!FrameProbe.isLeafModifier(""))
}
