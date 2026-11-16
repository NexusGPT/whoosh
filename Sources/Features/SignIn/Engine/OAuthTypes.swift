import Foundation

// The sign-in engine: pure Foundation + CryptoKit, no app types, so a sub-package can compile and test it alone.
// Standard parts it follows: RFC 6749 (authorization code), RFC 7636 (PKCE, S256), the AppAuth-iOS flow shape
// (Apache-2.0, read only) and MSAL's scope and error vocabulary (MIT, not linked: Package.swift is frozen,
// see docs/lanes/signin.md "## Contract deltas").

public enum SignInProvider: String, Sendable, CaseIterable {
    case microsoft, fastmail, google, chatgpt
}

/// Where a route talks to. Real hosts are public documentation; tests pass 127.0.0.1 URLs instead.
public struct OAuthEndpoints: Sendable, Equatable {
    public var authorize: URL
    public var token: URL
    public var clientId: String
    public var redirectScheme: String
    public var scopes: [String]
    public var extraAuthorizeParams: [String: String]
    public init(authorize: URL, token: URL, clientId: String, redirectScheme: String = "whoosh",
                scopes: [String], extraAuthorizeParams: [String: String] = [:]) {
        self.authorize = authorize; self.token = token; self.clientId = clientId
        self.redirectScheme = redirectScheme; self.scopes = scopes; self.extraAuthorizeParams = extraAuthorizeParams
    }
    public var redirectURI: String { "\(redirectScheme):/oauth/callback" }
}

public enum OAuthPresets {
    /// IMAP.AccessAsUser.All for mail over IMAP, Graph scopes for the Graph adapter, offline_access for the refresh token.
    public static let microsoftScopes = [
        "https://outlook.office.com/IMAP.AccessAsUser.All",
        "https://graph.microsoft.com/Mail.ReadWrite", "https://graph.microsoft.com/Mail.Send",
        "offline_access",
    ]
    /// JMAP scopes Fastmail documents for third-party apps (Authorization Code with PKCE).
    public static let fastmailScopes = [
        "urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail", "urn:ietf:params:jmap:submission",
    ]
    /// `clientId` is the app registration (Entra / Fastmail / OpenAI): a value the owner supplies, never a secret.
    public static func microsoft(clientId: String) -> OAuthEndpoints {
        OAuthEndpoints(authorize: URL(string: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize")!,
                       token: URL(string: "https://login.microsoftonline.com/common/oauth2/v2.0/token")!,
                       clientId: clientId, scopes: microsoftScopes, extraAuthorizeParams: ["prompt": "select_account"])
    }
    public static func fastmail(clientId: String) -> OAuthEndpoints {
        OAuthEndpoints(authorize: URL(string: "https://api.fastmail.com/oauth/authorize")!,
                       token: URL(string: "https://api.fastmail.com/oauth/refresh")!,
                       clientId: clientId, scopes: fastmailScopes)
    }
    public static func google(clientId: String) -> OAuthEndpoints {
        OAuthEndpoints(authorize: URL(string: "https://accounts.google.com/o/oauth2/v2/auth")!,
                       token: URL(string: "https://oauth2.googleapis.com/token")!,
                       clientId: clientId, scopes: ["https://www.googleapis.com/auth/gmail.modify"],
                       extraAuthorizeParams: ["access_type": "offline"])
    }
    /// The Codex-style ChatGPT login: authorization code + PKCE against OpenAI's auth host.
    public static func chatgpt(clientId: String) -> OAuthEndpoints {
        OAuthEndpoints(authorize: URL(string: "https://auth.openai.com/oauth/authorize")!,
                       token: URL(string: "https://auth.openai.com/oauth/token")!,
                       clientId: clientId, scopes: ["openid", "profile", "email", "offline_access"])
    }
}

/// Feature flags the sign-in routes read. The Google route is closed until Google's restricted-scope
/// verification passes (the same flag the Gmail API adapter reads, docs/plan.json fn-sync-gmail-api).
public struct SignInFlags: Sendable, Equatable {
    public var gmailAPIVerified: Bool
    public init(gmailAPIVerified: Bool = false) { self.gmailAPIVerified = gmailAPIVerified }
}

/// What a sign-in leaves behind. The token is already in the vault; only the reference travels on.
public struct SignInResult: Sendable, Equatable {
    public let provider: SignInProvider
    public let reference: VaultReference
    public let scopes: [String]
    public let expiresAtMs: Int64?
}

public enum SignInError: Error, Sendable, Equatable {
    /// words the owner reads as they are
    case words(String)
    case adminConsentBlocked
    case cancelled
    case routeClosed(String)
    public var ownerWords: String {
        switch self {
        case .words(let s), .routeClosed(let s): return s
        case .adminConsentBlocked: return SignInWords.adminBlocked
        case .cancelled: return SignInWords.cancelled
        }
    }
}

/// Every line the sign-in routes show, in one place.
public enum SignInWords {
    public static let adminBlocked =
        "Your organisation's admin has blocked this sign-in. Ask your admin to approve Whoosh for Microsoft 365 mail (IMAP.AccessAsUser.All and Mail.ReadWrite), or to allow users to consent to apps, then sign in again."
    public static let cancelled = "Sign-in was cancelled. Nothing was saved."
    public static let googleClosed = "Google sign-in is not open yet: Google's review of Whoosh is still pending. Use an app password for now."
    public static let chatgptNote = "Signing in with ChatGPT uses your subscription. OpenAI publishes no policy for this, so it can stop working at any time."
    public static let noRegistration = "This sign-in needs Whoosh's app registration, which is not set up on this build."
    /// The engine's own words for a refused address; the app passes the catalogue's (`RigNet.refusedWords`) instead.
    public static let offThisMac = "This test copy reaches only addresses on this Mac, so nothing was sent."
}
