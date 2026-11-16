/// The library's named families, in the order the mark editor lists them.
public enum EmblemFamily: String, CaseIterable, Sendable {
    case work, home, money, travel, people, tools, play, mail, places, health, learning, nature, food, transport, science, time
}

/// The emblem icon library: at least ICON_LIBRARY_MIN glyphs (docs/CONSTANTS.md), each with a detailed and a simplified
/// drawing. The 24 board glyphs are lifted verbatim from docs/design/boards/badges/emblem; the rest follow its grammar.
public enum EmblemLibrary {
    public static let all: [EmblemGlyph] = EmblemFamily.allCases.flatMap(glyphs(in:))

    public static func glyphs(in family: EmblemFamily) -> [EmblemGlyph] {
        switch family {
        case .work: return EmblemWork.glyphs
        case .home: return EmblemHome.glyphs
        case .money: return EmblemMoney.glyphs
        case .travel: return EmblemTravel.glyphs
        case .people: return EmblemPeople.glyphs
        case .tools: return EmblemTools.glyphs
        case .play: return EmblemPlay.glyphs
        case .mail: return EmblemMail.glyphs
        case .places: return EmblemPlaces.glyphs
        case .health: return EmblemHealth.glyphs
        case .learning: return EmblemLearning.glyphs
        case .nature: return EmblemNature.glyphs
        case .food: return EmblemFood.glyphs
        case .transport: return EmblemTransport.glyphs
        case .science: return EmblemScience.glyphs
        case .time: return EmblemTime.glyphs
        }
    }

    public static func glyph(_ id: String) -> EmblemGlyph? { all.first { $0.id == id } }
    /// The glyph an account shows until the owner picks one: the paper plane (the mail family's lead).
    public static var fallback: EmblemGlyph { glyph("paper-plane") ?? all[0] }
}

extension EmblemLibrary {
    /// The 20 glyph ids the ring library (AccountMarkIcon.libraryIds) stored, mapped onto emblem ids.
    static let aliases: [String: String] = [
        "chart": "bar-chart", "pen": "pencil", "globe": "globe", "bank": "bank", "sail": "sailboat", "mail": "envelope",
        "cup": "coffee", "plane": "airplane", "code": "toolbox", "phone": "chat", "leaf": "leaf", "book": "book",
    ]
    /// An account's icon id to its glyph: the id itself, then a ring-library alias, then the fallback.
    public static func resolve(_ id: String?) -> EmblemGlyph {
        guard let id else { return fallback }
        return glyph(id) ?? aliases[id].flatMap(glyph) ?? fallback
    }
}
