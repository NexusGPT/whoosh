/// The signin lane: OAuth sign-in for Microsoft, Fastmail, Google (closed behind gmail_api_verified) and ChatGPT.
/// The engine is in Engine/ (pure Foundation, proven alone in _captures/signin/proof); this file registers the
/// controls the Welcome screen presses. Tokens go to the vault seam only, never to the database.
public enum SignInFeature: FeatureModule {
    public static let id = "signin"

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        [
            "mac-welcome-accounts.sign-in-microsoft": { m in SignInController.shared.beginMailSignIn(.microsoft, model: m) },
            "mac-welcome-accounts.sign-in": { m in
                let c = SignInController.shared
                switch SignInController.provider(forAddress: c.address) {
                case .fastmail: return c.beginMailSignIn(.fastmail, model: m)
                case .microsoft: return c.beginMailSignIn(.microsoft, model: m)
                case .google: return c.beginMailSignIn(.google, model: m)
                default: return false
                }
            },
        ]
    }
}
