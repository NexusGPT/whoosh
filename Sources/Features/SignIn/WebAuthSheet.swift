#if canImport(AuthenticationServices)
import AuthenticationServices
import Foundation

/// The real sign-in sheet: ASWebAuthenticationSession (system browser sheet, shares the owner's browser sign-in).
/// Never used in tests; the tests pass a stand-in that answers like the stubbed provider.
public final class WebAuthSheet: NSObject, AuthorizationUI, ASWebAuthenticationPresentationContextProviding, @unchecked Sendable {
    public override init() { super.init() }

    public func authorize(_ url: URL, callbackScheme: String) async throws -> URL {
        try await withCheckedThrowingContinuation { cont in
            DispatchQueue.main.async {
                let session = ASWebAuthenticationSession(url: url, callbackURLScheme: callbackScheme) { callback, error in
                    if let callback { cont.resume(returning: callback); return }
                    if let e = error as? ASWebAuthenticationSessionError, e.code == .canceledLogin { cont.resume(throwing: SignInError.cancelled) }
                    else { cont.resume(throwing: SignInError.words(error?.localizedDescription ?? "The sign-in sheet closed without an answer.")) }
                }
                session.presentationContextProvider = self
                session.prefersEphemeralWebBrowserSession = false
                if !session.start() { cont.resume(throwing: SignInError.words("The sign-in sheet could not open.")) }
            }
        }
    }

    public func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor { ASPresentationAnchor() }
}
#endif
