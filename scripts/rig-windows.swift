// What the WINDOW SERVER shows of one process: the instrument of the hidden rig's proof (scripts/hidden-check.py).
// It reads from outside the app, so the app cannot vouch for itself.
//
//   rig-windows <pid>                one JSON object: {windows: [...], shown: n, front_pid, dock_tile}
//   rig-windows <pid> --watch <file|-> one JSON line per sample (every --every ms, default 20) until the pid ends
//                                    or the process is stopped; a line is written only when it differs from the last
//
// A window is SHOWN when the window server has it on screen, at an alpha above zero, over an active display.
// front_pid is the frontmost app (the one that gets the keyboard); dock_tile is true when the pid's activation
// policy is regular (a Dock tile and a menu bar).
import AppKit

func displays() -> [CGRect] {
    var n: UInt32 = 0
    CGGetActiveDisplayList(0, nil, &n)
    var ids = [CGDirectDisplayID](repeating: 0, count: Int(n))
    CGGetActiveDisplayList(n, &ids, &n)
    return ids.map { CGDisplayBounds($0) }
}

func sample(_ pid: pid_t) -> [String: Any] {
    let screens = displays()
    let everything = CommandLine.arguments.contains("--all")   // also windows the server holds off screen (a diagnostic)
    let all = CGWindowListCopyWindowInfo(everything ? [.optionAll] : [.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    var rows: [[String: Any]] = []
    var shown = 0
    for w in all where (w[kCGWindowOwnerPID as String] as? Int).map({ pid_t($0) }) == pid {
        let b = w[kCGWindowBounds as String] as? [String: CGFloat] ?? [:]
        let r = CGRect(x: b["X"] ?? 0, y: b["Y"] ?? 0, width: b["Width"] ?? 0, height: b["Height"] ?? 0)
        let alpha = w[kCGWindowAlpha as String] as? Double ?? 1
        let on = (w[kCGWindowIsOnscreen as String] as? Bool) ?? false
        let isShown = on && alpha > 0 && r.width > 0 && r.height > 0 && screens.contains { $0.intersects(r) }
        if isShown { shown += 1 }
        rows.append(["id": w[kCGWindowNumber as String] ?? 0, "name": w[kCGWindowName as String] ?? "", "layer": w[kCGWindowLayer as String] ?? 0,
                     "alpha": alpha, "onscreen": on, "x": r.minX, "y": r.minY, "w": r.width, "h": r.height, "shown": isShown])
    }
    let app = NSRunningApplication(processIdentifier: pid)
    return ["t": Date().timeIntervalSince1970, "windows": rows, "shown": shown,
            "front_pid": Int(NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1),
            "dock_tile": app?.activationPolicy == .regular, "alive": app != nil && !(app?.isTerminated ?? true)]
}

func json(_ o: Any) -> String {
    guard let d = try? JSONSerialization.data(withJSONObject: o, options: [.sortedKeys]) else { return "{}" }
    return String(decoding: d, as: UTF8.self)
}

let args = CommandLine.arguments
guard args.count >= 2, let pid = pid_t(args[1]) else {
    FileHandle.standardError.write(Data("usage: rig-windows <pid> [--watch <file> [--every <ms>]]\n".utf8)); exit(2)
}
if let i = args.firstIndex(of: "--watch"), i + 1 < args.count {
    let every = args.firstIndex(of: "--every").flatMap { $0 + 1 < args.count ? Double(args[$0 + 1]) : nil } ?? 20
    let out: FileHandle
    if args[i + 1] == "-" { out = FileHandle.standardOutput } else {
        FileManager.default.createFile(atPath: args[i + 1], contents: nil)
        guard let h = FileHandle(forWritingAtPath: args[i + 1]) else { exit(2) }
        out = h
    }
    var last = ""
    var n = 0
    while kill(pid, 0) == 0 {
        var s = sample(pid)
        n += 1
        s["n"] = n   // samples taken so far, the written ones included
        let key = json(["w": s["windows"] ?? [], "f": s["front_pid"] ?? 0, "d": s["dock_tile"] ?? false])
        if key != last { last = key; out.write(Data((json(s) + "\n").utf8)) }
        s.removeAll()
        usleep(useconds_t(every * 1000))
    }
    out.write(Data((json(["t": Date().timeIntervalSince1970, "ended": true, "n": n]) + "\n").utf8))
} else {
    print(json(sample(pid)))
}
