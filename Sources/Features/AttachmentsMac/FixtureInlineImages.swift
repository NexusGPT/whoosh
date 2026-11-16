import Foundation

/// The fixture corpus's inline images: in an escaped body paragraph, "[[inline:NAME]]" becomes an <img> of the cid:
/// part NAME and "[[inline-data:BASE64]]" an <img> of a data: PNG, as a mail with images placed in its body has.
/// Called by the fixture core's body (hook commit "attachments hook: fixturecore"); nothing else changes in the text.
public enum FixtureInlineImages {
    public static func html(_ escaped: String) -> String {
        var out = escaped
        if let re = try? NSRegularExpression(pattern: #"\[\[inline:([^\]\s]+)\]\]"#) {
            out = re.stringByReplacingMatches(in: out, range: NSRange(out.startIndex..., in: out),
                                              withTemplate: "<img src=\"cid:$1\" alt=\"$1\">")
        }
        if let re = try? NSRegularExpression(pattern: #"\[\[inline-data:([A-Za-z0-9+/=]+)\]\]"#) {
            out = re.stringByReplacingMatches(in: out, range: NSRange(out.startIndex..., in: out),
                                              withTemplate: "<img src=\"data:image/png;base64,$1\" alt=\"\">")
        }
        return out
    }
}
