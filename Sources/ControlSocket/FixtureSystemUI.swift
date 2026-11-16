import Foundation
#if os(macOS)
import AppKit
#endif

/// `fixture system-ui <kind>`: makes the REAL framework call for one kind of system UI, so the hidden proof shows that
/// the hidden rig refuses it (HiddenSystemUI.swift) while the window server is watched. Kinds: quick-look, menu,
/// save-panel, alert, share; `hooks` lists every hooked entry point. Refused outside a hidden rig: on a shown copy
/// these calls would put the system's panels on the screen. `open` and `notification` are not driven (a missed hook
/// would open another app or post a banner): `hooks` shows they are installed.
/// `core state system-ui/<kind>` answers how many times it was asked; `system-ui/<kind>/<n>` what the n-th ask was.
@MainActor
enum FixtureSystemUI {
    static func run(_ kind: String) throws -> [String: Any] {
        #if os(macOS)
        guard HiddenRig.isOn else { throw ControlFailure("fixture system-ui runs only on a hidden rig") }
        if kind == "hooks" { return ["hooks": HiddenRig.hooked] }
        let view = WindowBridge.mainWindow()?.contentView
        switch kind {
        case "quick-look":
            guard let cls = NSClassFromString("QLPreviewPanel") as? NSObject.Type,
                  let panel = cls.perform(NSSelectorFromString("sharedPreviewPanel"))?.takeUnretainedValue() as? NSWindow else {
                throw ControlFailure("Quick Look is not available here")
            }
            panel.makeKeyAndOrderFront(nil)
        case "menu":
            let menu = NSMenu(title: "probe")
            menu.addItem(withTitle: "probe", action: nil, keyEquivalent: "")
            guard let v = view, let e = NSEvent.mouseEvent(with: .rightMouseDown, location: NSPoint(x: 40, y: 40), modifierFlags: [], timestamp: 0,
                                                           windowNumber: v.window?.windowNumber ?? 0, context: nil, eventNumber: 0, clickCount: 1, pressure: 1) else {
                throw ControlFailure("no main window for the menu")
            }
            NSMenu.popUpContextMenu(menu, with: e, for: v)
            _ = menu.popUp(positioning: nil, at: NSPoint(x: 40, y: 40), in: v)
        case "save-panel":
            NSSavePanel().begin { _ in }
            _ = NSOpenPanel().runModal()
        case "alert":
            _ = NSAlert().runModal()
        case "share":
            guard let v = view else { throw ControlFailure("no main window for the picker") }
            NSSharingServicePicker(items: ["probe"]).show(relativeTo: .zero, of: v, preferredEdge: .minY)
        default:
            throw ControlFailure("fixture system-ui takes quick-look, menu, save-panel, alert, share or hooks")
        }
        return ["kind": kind, "refused": HiddenRig.systemUI.mapValues(\.count)]
        #else
        throw ControlFailure("system UI is a Mac concern")
        #endif
    }

    static func state(_ path: String) throws -> [String: Any] {
        #if os(macOS)
        let parts = path.split(separator: "/").map(String.init)   // system-ui/<kind>[/<n>]
        guard parts.count >= 2 else { throw ControlFailure("usage: core state system-ui/<kind>[/<n>]") }
        let asks = HiddenRig.systemUI[parts[1]] ?? []
        if parts.count == 2 { return ["value": asks.count] }
        guard let n = Int(parts[2]), asks.indices.contains(n) else { throw ControlFailure("no ask \(parts[2]) of \(parts[1])") }
        return ["value": asks[n]]
        #else
        throw ControlFailure("system UI is a Mac concern")
        #endif
    }
}
