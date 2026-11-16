import Foundation

/// Every line the install step says, through the 14-language catalogue (the key is the English text).
enum OnDeviceWords {
    static var title: String { Words.text("Setting up on-device AI") }
    static var lead: String { Words.text("One model runs on this Mac for Ask, Writer and summaries. It downloads once and nothing else leaves your Mac.") }
    static var install: String { Words.text("Set up on-device AI") }
    static var pause: String { Words.text("Pause") }
    static var resume: String { Words.text("Resume") }
    static var cancel: String { Words.text("Cancel") }
    static var retry: String { Words.text("Try again") }
    static var close: String { Words.text("Done") }
    static var readyTitle: String { Words.text("On-device AI is ready") }
    static var tryTitle: String { Words.text("Try it") }
    static var tryPlaceholder: String { Words.text("Ask the on-device model anything") }
    static var ask: String { Words.text("Ask") }
    static var thinking: String { Words.text("Thinking on this Mac.") }
    static var modelLabel: String { Words.text("Model") }
    static var sizeLabel: String { Words.text("Download") }
    static var licenceLabel: String { Words.text("Licence") }
    static var whereLabel: String { Words.text("Stays") }
    static var whereValue: String { Words.text("On this Mac, in Application Support") }
    static var notInstalledAnswer: String { Words.text("On-device model not installed yet. Set up on-device AI first.") }
    static func percent(_ f: Double) -> String { "\(Int((f * 100).rounded())) %" }
    static func of(_ r: Int64, _ t: Int64) -> String { Words.text("%@ of %@", ModelInstallState.size(r), ModelInstallState.size(t)) }
}
