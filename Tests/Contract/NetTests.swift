import Foundation
import Testing

extension Contract {
    /// `net`: the planned verb for network checks. The LIVE contract (docs/CONTROL-SOCKET.md) has NO `net` verb, and the
    /// standing rule is "Nothing leaves this Mac". This file encodes the live contract: `net` is unknown, in every
    /// spelling, and the registry lists the verbs the doc lists. A `net` verb added later must arrive with its doc row
    /// and its own test, so these go red then (finding in docs/lanes/contracttests.md).
    @Suite("net") struct NetVerb {
        @Test func netIsAnUnknownVerb() throws {
            for args in [[], ["state"], ["http://example.invalid"], ["off"]] as [[Any]] {
                let r = try Ctl.ask("net", args)
                #expect(!r.ok)
                #expect(r.error == "unknown verb net", "net \(args): \(r.error ?? "")")
            }
        }

        @Test func theRegistryDoesNotListNetAndDoesListTheDocumentedVerbs() throws {
            let verbs = try #require(try Ctl.ok("verbs").fields["verbs"] as? [String])
            #expect(!verbs.contains("net"))
            #expect(verbs.contains("clock") && verbs.contains("shot"), "positive control: the probe reads a populated registry")
        }
    }
}
