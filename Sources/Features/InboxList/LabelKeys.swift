import Foundation

/// The Superhuman label keys on the focused thread: Y removes the label in view, [ removes it and steps to the next thread,
/// ] removes it and steps to the previous, ⇧Y removes every label. The labels lane owns the doors (`LabelDoors`, delta 10:
/// `removeLabel` / `removeAllLabels`, one undo each); this list only routes its keys there.
@MainActor
enum LabelKeys {
    /// `step` 0 stays on the row, +1 goes to the next thread after the removal, -1 to the previous one.
    static func remove(_ m: AppModel, step: Int, all: Bool = false) -> Bool {
        LabelDoors.remove(m, step: step, all: all)
    }
}
