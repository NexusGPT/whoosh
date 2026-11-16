import Foundation

/// One read in flight per store. A refetch asked for while one runs, or a push that lands during
/// one, marks the store dirty and the read runs once more, so a caller who awaits `run` always
/// returns after a read that began after its call. Nothing is dropped, nothing is blanked.
@MainActor
final class Coalescer {
    private var inflight: Task<Void, Never>?
    private var dirty = false

    /// A push arrived: an unfinished read may be older than it, so read once more.
    func markDirty() { if inflight != nil { dirty = true } }

    func run(_ body: @escaping @MainActor () async -> Void) async {
        if let t = inflight {
            dirty = true
            await t.value
            return
        }
        let t = Task { @MainActor [self] in
            repeat {
                dirty = false
                await body()
            } while dirty
            inflight = nil
        }
        inflight = t
        await t.value
    }
}
