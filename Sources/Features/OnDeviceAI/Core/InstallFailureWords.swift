import Foundation

extension ModelInstaller {
    static func words(for error: Error) -> String {
        if let f = error as? FileFetcher.Failure {
            switch f {
            case .status(let c): return Words.text("The download server answered %@. Tap Try again.", c)
            case .rangeRefused: return Words.text("The download server refused to resume. Tap Try again to start the file over.")
            case .io(let w): return Words.text("The model could not be written to disk: %@", w)
            }
        }
        let code = (error as? URLError)?.code
        if code == .notConnectedToInternet || code == .networkConnectionLost || code == .timedOut || code == .cannotConnectToHost {
            return Words.text("The download stopped because the connection dropped. Connect, then tap Resume: it continues where it stopped.")
        }
        return Words.text("The download failed: %@ Tap Try again.", error.localizedDescription)
    }
}
