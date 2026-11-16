import Foundation
import Testing

extension Contract {
    /// `type <text>`: appends to the focused text input; no reply fields (docs/CONTROL-SOCKET.md).
    /// The text lands in `AppModel.typed`, which no verb reads back at this commit: the append itself is
    /// UNOBSERVABLE on the socket (finding in docs/lanes/contracttests.md). What the verb promises on the wire is tested.
    @Suite("type") struct TypeVerb {
        @Test func typedTextIsAcceptedWithNoReplyFields() throws {
            let r = try Ctl.ask("type", ["hello"])
            #expect(r.ok, "type hello: \(r.error ?? "")")
            #expect(r.keys == ["ok"], "type has no reply fields, got \(r.keys.sorted())")
        }

        @Test func noArgumentIsRefusedInWords() throws {
            let r = try Ctl.ask("type")
            #expect(!r.ok)
            #expect(r.error == "type needs the text")
        }

        @Test func aNumberIsText() throws {
            let r = try Ctl.ask("type", [42])
            #expect(r.ok, "a number argument is text: \(r.error ?? "")")
        }

        @Test func anObjectArgumentIsRefused() throws {
            let r = try Ctl.raw(#"{"verb":"type","args":[{"a":1}]}"#)
            #expect(r["ok"] as? Bool == false)
            #expect(r["error"] as? String == "argument 0 of type is neither text nor a number")
        }

        @Test func emptyAndVeryLongTextAreBothAccepted() throws {
            #expect(try Ctl.ask("type", [""]).ok)
            let long = String(repeating: "ü", count: 100_000)
            let r = try Ctl.ask("type", [long])
            #expect(r.ok, "100k characters: \(r.error ?? "")")
            #expect(r.keys == ["ok"])
        }

        @Test func theVerbIsRegistered() throws {
            // the `verbs` registry names it; a renamed verb answers "unknown verb type"
            let verbs = try Ctl.ok("verbs").fields["verbs"] as? [String] ?? []
            #expect(verbs.contains("type"))
            #expect(verbs.contains("verbs"), "positive control: the registry lists itself")
        }
    }
}
