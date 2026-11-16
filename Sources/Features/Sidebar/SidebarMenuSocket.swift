import Foundation

/// Round 5's socket surface for the right-click menus, the "+" on Labels and their dialogs. The same SidebarState functions
/// back the real menu buttons, so a driven check exercises what a click does.
extension SidebarFeature {
    static func menuRows(_ st: SidebarState) -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-main.sidebar.menus": { _ in
                st.allSubjects().flatMap { s in
                    st.menu(s).map { i in
                        var r: ControlRow = ["ax": "mac-main.sidebar.menu.\(s.key).\(i.id)", "subject": s.key, "item": i.id, "title": i.title,
                                             "enabled": i.enabled, "destructive": i.destructive]
                        if let n = i.needs { r["needs"] = n }
                        return r
                    }
                }
            },
            "mac-main.sidebar.prompt": { _ in
                [["open": st.prompt != nil, "title": st.prompt?.title ?? "", "text": st.promptText, "has_rule": st.prompt?.hasRule ?? false]]
            },
            "mac-main.sidebar.confirm": { _ in
                [["open": st.deleteAsk != nil, "name": st.deleteAsk?.name ?? "", "noun": st.deleteAsk?.noun ?? ""]]
            },
            "mac-main.sidebar.labels-header": { _ in [["plus": true, "hover": st.hoverHeader == "labels"]] },
        ]
    }

    static func menuClicks(_ st: SidebarState) -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "mac-main.sidebar.labels.add": { _ in LabelInspector.openNew(parent: nil) },
            /// The drive cannot hover: this forces the header's hover state, which shows the "+".
            "mac-main.sidebar.labels.hover": { _ in st.hoverHeader = st.hoverHeader == "labels" ? nil : "labels"; return true },
            "mac-main.sidebar.prompt.confirm": { m in let typed = m.typed; m.typed = ""; return st.confirmPrompt(model: m, typed: typed) },
            "mac-main.sidebar.prompt.cancel": { _ in guard st.prompt != nil else { return false }; st.cancelPrompt(); return true },
            "mac-main.sidebar.confirm.yes": { m in st.confirmDelete(model: m) },
            "mac-main.sidebar.confirm.no": { _ in guard st.deleteAsk != nil else { return false }; st.cancelDelete(); return true },
        ]
        for s in st.allSubjects() {
            for i in st.menu(s) where i.enabled { t["mac-main.sidebar.menu.\(s.key).\(i.id)"] = i.run }
        }
        return t
    }
}
