import Foundation

/// The one rule for an account's NAME on screen. The core makes a local catch-all account for imported mail no address
/// matches; its stored name is the English "Imported". The owner's rule is "don't mix languages", so that one account's
/// name goes through the catalogue wherever an account is named; every other account shows the name the owner gave it.
/// The address and the id are never touched.
public enum AccountNames {
    /// The address of the catch-all account. Two files hold it: `core/crates/engine/src/import_run.rs` (IMPORTED_ADDRESS,
    /// which creates the account) and this one (which recognises it). Change both together.
    public static let importedAddress = "imported@whoosh.local"

    /// The name to draw: the catalogue's "Imported" for the catch-all account, else the stored name.
    public static func display(address: String, name: String) -> String {
        address == importedAddress ? Words.text("Imported") : name
    }

    /// The `Account` of the sidebar, the composer and the emblems.
    public static func display(_ a: Account) -> String { display(address: a.address, name: a.displayName) }

    /// The core's `AccountSummary` (the account list the quick reply, the notch and the sync status read).
    public static func display(_ a: AccountSummary) -> String { display(address: a.address, name: a.name) }

    /// The summary with its name already localised, for a model that keeps the core's list and draws `.name` in many views.
    public static func localised(_ a: AccountSummary) -> AccountSummary {
        var b = a
        b.name = display(a)
        return b
    }
}
