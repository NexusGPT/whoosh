import Foundation

/// The few numbers this lane needs. Each one is a model fact, not a product choice, and each is proposed
/// as a docs/CONSTANTS.md row in docs/lanes/ondeviceai.md ("Contract deltas") because that file is frozen
/// for this lane. Everything else the Laya runtime needs (sequence length, head length, temperatures,
/// layer count, heads) is READ from the downloaded checkpoint's own config files, never typed here.
public enum OnDeviceLimits {
    /// ON_DEVICE_CONTEXT_TOKENS: Apple's on-device language model window (prompt plus answer).
    public static let contextTokens = 4_096
    /// ON_DEVICE_CHARS_PER_TOKEN: a deliberately pessimistic estimate (real text averages 3 to 4 characters
    /// per token); a prompt is refused when even the optimistic count cannot fit, so a wrong guess never
    /// lets an over-long prompt through for ordinary mail text.
    public static let charsPerTokenFloor = 2.5
    /// LAYA_OPTION_TOKEN_CAP: the authors' code cuts each rendered option to 48 tokens (rl_common.py build_sequence).
    public static let layaOptionTokenCap = 48
    /// LAYA_MIN_OPTION_BUDGET and LAYA_MIN_HEAD_TOKENS: the authors' shrink thresholds (same function).
    public static let layaOptionBudgetFloor = 16
    public static let layaMinHeadTokens = 8
}
