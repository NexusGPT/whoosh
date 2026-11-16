import Foundation
import Speech

/// Speech on device: SFSpeechRecognizer with on-device recognition required, so no audio leaves this Mac. A refused permission or a
/// language the Mac cannot recognise on device comes back as `.refused` with words the owner can act on; the sound is still
/// embedded and searched by name. Needs NSSpeechRecognitionUsageDescription in the app's Info.plist (door: the app shell).
public struct OnDeviceSpeech: SpeechTranscribing {
    private let limits: ReadingLimits
    private let locale: Locale
    private let authorization: @Sendable () -> SFSpeechRecognizerAuthorizationStatus

    /// `authorization` is a seam for checks; the default asks the system (once, the first time).
    public init(limits: ReadingLimits, locale: Locale = .current, authorization: @escaping @Sendable () -> SFSpeechRecognizerAuthorizationStatus = { OnDeviceSpeech.systemAuthorization() }) {
        self.limits = limits
        self.locale = locale
        self.authorization = authorization
    }

    public static func systemAuthorization() -> SFSpeechRecognizerAuthorizationStatus {
        let status = SFSpeechRecognizer.authorizationStatus()
        guard status == .notDetermined else { return status }
        final class Box: @unchecked Sendable { var status = SFSpeechRecognizerAuthorizationStatus.notDetermined }
        let box = Box(), done = DispatchSemaphore(value: 0)
        SFSpeechRecognizer.requestAuthorization { box.status = $0; done.signal() }
        done.wait()
        return box.status
    }

    public func transcribe(at url: URL, fromVideo: Bool) throws -> SpeechOutcome {
        switch authorization() {
        case .authorized: break
        case .denied: return .refused(reasonWords: "Speech recognition is off for Whoosh. Allow it in System Settings, Privacy & Security, Speech Recognition, to search inside voice notes and videos.")
        case .restricted: return .refused(reasonWords: "Speech recognition is restricted on this Mac, so voice notes and videos are searched by name only.")
        default: return .refused(reasonWords: "Speech recognition has not been allowed yet, so voice notes and videos are searched by name only.")
        }
        guard let recognizer = SFSpeechRecognizer(locale: locale), recognizer.supportsOnDeviceRecognition else {
            return .refused(reasonWords: "On-device speech recognition is not available for \(locale.identifier) on this Mac, so voice notes and videos are searched by name only.")
        }
        let request = SFSpeechURLRecognitionRequest(url: url)
        request.requiresOnDeviceRecognition = true
        request.shouldReportPartialResults = false
        final class Box: @unchecked Sendable { var text: String?; var error: Error? }
        let box = Box(), done = DispatchSemaphore(value: 0)
        let task = recognizer.recognitionTask(with: request) { result, error in
            if let result, result.isFinal { box.text = result.bestTranscription.formattedString; done.signal() } else if let error { box.error = error; done.signal() }
        }
        if done.wait(timeout: .now() + limits.speechTimeoutSeconds) == .timedOut {
            task.cancel()
            throw AttachmentReadError.unreadable("speech recognition took too long on this file")
        }
        if let error = box.error {
            let e = error as NSError
            if e.domain == "kAFAssistantErrorDomain" && e.code == 1110 { return .noSpeech }
            throw AttachmentReadError.unreadable("speech could not be recognised: \(e.localizedDescription)")
        }
        let text = (box.text ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? .noSpeech : .text(text)
    }
}
