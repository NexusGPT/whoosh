import Foundation
import Testing
@testable import WhooshUI

// A hidden test copy (a rig) loads no picture from a host outside this Mac, even after "Load once" (key I) (owner: "nothing leaves this
// Mac except the relay"). Found 2026-11-11 (rig network audit, gap 3): the fixture's sample mails name https://example.com/banner.png,
// t1.gif and t2.gif, and a hidden run that pressed I asked example.com. `RigNet` is the one rule. No test opens a socket or a web view.

private let rigEnv = ["WHOOSH_RIG_HIDDEN": "1"]
private let ownerEnv: [String: String] = [:]

private func blocked(_ src: String) -> String { "<img src=\"cid:whoosh-blocked\" data-whoosh-src=\"\(src)\" alt=\"\">" }
private let remote = ["https://example.com/banner.png", "https://example.com/t1.gif", "https://example.com/t2.gif"]

@MainActor @Test func aRigDrawsNoRemotePictureAfterLoadOnce() {
    let html = remote.map(blocked).joined()
    let out = MessageBodyDocument.placeholderImages(html, loadPictures: true, env: rigEnv)
    #expect(!out.contains("<img src=\"https://"))                 // the hidden read `fetchable` counts exactly this
    #expect(!out.contains("example.com"))
    #expect(out.components(separatedBy: MessageBodyWords.pictureNotLoaded).count - 1 >= remote.count)   // each stays "not loaded"
}

@MainActor @Test func aRigDrawsAPictureServedFromThisMac() {
    let local = "https://127.0.0.1:8443/banner.png"
    let out = MessageBodyDocument.placeholderImages(blocked(local), loadPictures: true, env: rigEnv)
    #expect(out.contains("<img src=\"\(local)\""))
}

@MainActor @Test func aRigsPicturesRuleListBlocksEveryHostButThisMac() throws {
    let json = MessageBodyPolicy.picturesRuleListJSON(rig: true)
    let rules = try #require(try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [[String: Any]])
    let actions = rules.compactMap { ($0["action"] as? [String: Any])?["type"] as? String }
    let filters = rules.compactMap { ($0["trigger"] as? [String: Any])?["url-filter"] as? String }
    #expect(filters.contains("^https?://") && actions.contains("block"))
    let exceptions = rules.filter { ($0["action"] as? [String: Any])?["type"] as? String == "ignore-previous-rules" }
    #expect(exceptions.count == 3)
    for e in exceptions {
        let f = try #require((e["trigger"] as? [String: Any])?["url-filter"] as? String)
        let re = try NSRegularExpression(pattern: f)
        for host in ["https://example.com/banner.png", "https://127.0.0.1.example.com/x.gif"] {
            #expect(re.firstMatch(in: host, range: NSRange(host.startIndex..., in: host)) == nil)
        }
        #expect((e["trigger"] as? [String: Any])?["resource-type"] as? [String] == ["image"])
    }
    #expect(!MessageBodyPolicy.rigPicturesContentSecurityPolicy.contains("https:;"))
}

@MainActor @Test func theOwnersCopyStillLoadsRemotePicturesWhenAsked() {
    let out = MessageBodyDocument.placeholderImages(blocked(remote[0]), loadPictures: true, env: ownerEnv)
    #expect(out.contains("<img src=\"\(remote[0])\""))
    // the owner's list, exactly as it was at 1542f4b1
    let before = #"[{"trigger":{"url-filter":"^https?://","resource-type":["document","style-sheet","script","font","raw","svg-document","media","popup"]},"action":{"type":"block"}},{"trigger":{"url-filter":"^wss?://"},"action":{"type":"block"}},{"trigger":{"url-filter":"^ftp://"},"action":{"type":"block"}}]"#
    #expect(MessageBodyPolicy.picturesRuleListJSON(rig: false) == before)
    #expect(MessageBodyPolicy.picturesContentSecurityPolicy == "default-src 'none'; img-src cid: data: https:; style-src 'unsafe-inline'")
}

@MainActor @Test func nothingLoadsBeforeTheOwnerAsksOnEitherCopy() {
    for env in [rigEnv, ownerEnv] {
        #expect(!MessageBodyDocument.placeholderImages(blocked(remote[0]), loadPictures: false, env: env).contains("<img src=\"https://"))
    }
}
