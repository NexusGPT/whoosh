import Foundation
import Testing

extension Contract {
    /// `verbs` and the request line itself: the registry equals the verbs table of docs/CONTROL-SOCKET.md.
    @Suite("verbs") struct VerbsVerb {
        /// The verbs the doc's table names, read from the doc itself.
        static func documented() throws -> Set<String> {
            let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            let doc = try String(contentsOf: root.appendingPathComponent("docs/CONTROL-SOCKET.md"), encoding: .utf8)
            var out = Set<String>()
            var inSection = false, started = false
            for line in doc.split(separator: "\n", omittingEmptySubsequences: false) {
                if line.hasPrefix("## Verbs") { inSection = true; continue }
                guard inSection else { continue }
                if line.hasPrefix("| `"), let end = line.dropFirst(3).firstIndex(of: "`") {
                    started = true
                    out.insert(String(line.dropFirst(3)[..<end]))
                } else if started && !line.hasPrefix("|") {
                    break   // the first table ends at the first line that is not a table row
                }
            }
            return out
        }

        @Test func theRegistryEqualsTheDocumentedVerbsExactly() throws {
            let doc = try Self.documented()
            #expect(doc.count >= 12, "positive control: the doc parse found its table, got \(doc.sorted())")
            let r = try Ctl.ok("verbs")
            #expect(r.keys == ["ok", "verbs"])
            let live = try #require(r.fields["verbs"] as? [String])
            #expect(live == live.sorted(), "the registry is sorted")
            #expect(Set(live) == doc, "live \(live) vs documented \(doc.sorted())")
        }

        @Test func malformedRequestLinesAreRefusedInWords() throws {
            let cases: [(String, String)] = [
                ("not json", "the request is not a JSON object"),
                ("[1,2]", "the request is not a JSON object"),
                (#"{"args":["x"]}"#, "the request has no verb"),
                (#"{"verb":""}"#, "the request has no verb"),
                (#"{"verb":7}"#, "the request has no verb"),
                (#"{"verb":"zzz"}"#, "unknown verb zzz"),
                (#"{"verb":"TYPE","args":["x"]}"#, "unknown verb TYPE"),
                (#"{"verb":"key","args":null}"#, "key needs a chord"),
            ]
            for (line, words) in cases {
                let r = try Ctl.raw(line)
                #expect(r["ok"] as? Bool == false, "\(line)")
                #expect(r["error"] as? String == words, "\(line): \(r["error"] ?? "nil")")
            }
            #expect(try Ctl.ask("verbs").ok, "positive control: the app still answers after garbage")
        }

        /// docs/CONTROL-SOCKET.md: "Arguments are text or numbers; anything else is refused." A JSON boolean is
        /// neither, but the server's NSNumber cast accepts it as the text "1"/"0". OPEN DIVERGENCE (finding): the known
        /// issue goes red the day the server refuses it, so the day is noticed.
        @Test func aBooleanArgumentIsRefusedPerTheDoc() throws {
            for lit in ["true", "false"] {
                let r = try Ctl.raw(#"{"verb":"key","args":[\#(lit)]}"#)
                withKnownIssue("server coerces JSON \(lit) into text instead of refusing it") {
                    #expect(r["error"] as? String == "argument 0 of key is neither text nor a number", "got \(r["error"] ?? "nil")")
                }
            }
            // the objects and lists ARE refused, so the refusal path is live
            let r = try Ctl.raw(#"{"verb":"key","args":[[1]]}"#)
            #expect(r["error"] as? String == "argument 0 of key is neither text nor a number")
        }
    }
}
