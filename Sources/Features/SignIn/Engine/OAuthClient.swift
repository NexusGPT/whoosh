import Foundation

/// Authorization-code exchange with PKCE (RFC 6749 §4.1.3, RFC 7636) and the provider error mapping.
public struct OAuthClient: Sendable {
    public let session: URLSession
    public init(session: URLSession = URLSession(configuration: .ephemeral)) { self.session = session }

    public func authorizeURL(_ e: OAuthEndpoints, pkce: PKCE, state: String) -> URL {
        var c = URLComponents(url: e.authorize, resolvingAgainstBaseURL: false)!
        var q: [URLQueryItem] = [
            .init(name: "response_type", value: "code"), .init(name: "client_id", value: e.clientId),
            .init(name: "redirect_uri", value: e.redirectURI), .init(name: "scope", value: e.scopes.joined(separator: " ")),
            .init(name: "state", value: state), .init(name: "code_challenge", value: pkce.challenge),
            .init(name: "code_challenge_method", value: "S256"),
        ]
        for (k, v) in e.extraAuthorizeParams.sorted(by: { $0.key < $1.key }) { q.append(.init(name: k, value: v)) }
        c.queryItems = q
        return c.url!
    }

    /// Reads the redirect the sign-in sheet ended on: the code, or the provider's refusal in words.
    public func code(from callback: URL, expectedState: String) throws -> String {
        let items = URLComponents(url: callback, resolvingAgainstBaseURL: false)?.queryItems ?? []
        func v(_ n: String) -> String? { items.first { $0.name == n }?.value }
        if let err = v("error") { throw OAuthClient.map(error: err, description: v("error_description")) }
        guard v("state") == expectedState else { throw SignInError.words("The sign-in answer did not match this request, so it was ignored.") }
        guard let code = v("code"), !code.isEmpty else { throw SignInError.words("The sign-in finished without a code.") }
        return code
    }

    public func exchange(code: String, endpoints e: OAuthEndpoints, pkce: PKCE, nowMs: Int64) async throws -> TokenSet {
        try await post(e.token, [
            "grant_type": "authorization_code", "code": code, "redirect_uri": e.redirectURI,
            "client_id": e.clientId, "code_verifier": pkce.verifier,
        ], nowMs: nowMs)
    }

    public func refresh(_ refreshToken: String, endpoints e: OAuthEndpoints, nowMs: Int64) async throws -> TokenSet {
        var t = try await post(e.token, ["grant_type": "refresh_token", "refresh_token": refreshToken, "client_id": e.clientId], nowMs: nowMs)
        if t.refreshToken == nil { t.refreshToken = refreshToken }
        return t
    }

    private func post(_ url: URL, _ form: [String: String], nowMs: Int64) async throws -> TokenSet {
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        var allowed = CharacterSet.alphanumerics; allowed.insert(charactersIn: "-._~")
        req.httpBody = Data(form.sorted { $0.key < $1.key }.map {
            "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: allowed) ?? "")"
        }.joined(separator: "&").utf8)
        let (data, resp): (Data, URLResponse)
        do { (data, resp) = try await session.data(for: req) }
        catch { throw SignInError.words("The sign-in server could not be reached: \(error.localizedDescription)") }
        let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
        let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        if !(200..<300).contains(status) || obj["error"] != nil {
            throw OAuthClient.map(error: obj["error"] as? String ?? "http_\(status)", description: obj["error_description"] as? String)
        }
        guard let access = obj["access_token"] as? String, !access.isEmpty else {
            throw SignInError.words("The sign-in server answered without an access token.")
        }
        let expires = (obj["expires_in"] as? Double) ?? (obj["expires_in"] as? String).flatMap(Double.init)
        return TokenSet(accessToken: access, refreshToken: obj["refresh_token"] as? String,
                        expiresAtMs: expires.map { nowMs + Int64($0 * 1000) }, scope: obj["scope"] as? String,
                        tokenType: obj["token_type"] as? String)
    }

    /// Microsoft's admin-consent refusals: AADSTS90094 (admin consent required), AADSTS65001 (no consent for the app),
    /// AADSTS90008 / AADSTS500202 are not consent and stay as the provider's words.
    public static func isAdminConsentBlock(error: String, description: String?) -> Bool {
        let d = (description ?? "").uppercased()
        return d.contains("AADSTS90094") || d.contains("AADSTS65001") || error == "consent_required" || error == "admin_consent_required"
    }

    public static func map(error: String, description: String?) -> SignInError {
        if isAdminConsentBlock(error: error, description: description) { return .adminConsentBlocked }
        if error == "access_denied" && (description ?? "").isEmpty { return .cancelled }
        return .words(description.map { $0.split(separator: "\r\n").first.map(String.init) ?? $0 } ?? "The provider refused the sign-in (\(error)).")
    }
}
