import SwiftUI
import UserNotifications
#if os(iOS) && canImport(VisionKit)
import VisionKit
#endif

/// The one ask for notifications. A test copy never reaches the system: it answers "skipped-in-test".
enum WelcomeNotifications {
    @MainActor static func request() async -> String {
        guard !WelcomeTestCopy.on else { return "skipped-in-test" }
        do {
            let ok = try await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .badge, .sound])
            return ok ? "allowed" : "declined"
        } catch {
            return "declined"
        }
    }
}

/// The system's own code scanner (VisionKit). `canScan` is false without a camera or when the camera is off for Whoosh;
/// the flow then says so instead of showing a dead frame. A test copy never builds this view.
enum WelcomeScanner {
    @MainActor static var canScan: Bool {
        #if os(iOS) && canImport(VisionKit)
        DataScannerViewController.isSupported && DataScannerViewController.isAvailable
        #else
        false
        #endif
    }
}

#if os(iOS) && canImport(VisionKit)
struct WelcomeScannerView: UIViewControllerRepresentable {
    var onCode: @MainActor (String) -> Void
    var onFail: @MainActor () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(onCode: onCode) }

    func makeUIViewController(context: Context) -> DataScannerViewController {
        let v = DataScannerViewController(recognizedDataTypes: [.barcode(symbologies: [.qr])], qualityLevel: .balanced,
                                          recognizesMultipleItems: false, isHighFrameRateTrackingEnabled: false,
                                          isPinchToZoomEnabled: false, isGuidanceEnabled: false, isHighlightingEnabled: false)
        v.delegate = context.coordinator
        return v
    }

    func updateUIViewController(_ v: DataScannerViewController, context: Context) {
        guard !v.isScanning else { return }
        do { try v.startScanning() } catch { Task { @MainActor in onFail() } }
    }

    @MainActor
    final class Coordinator: NSObject, DataScannerViewControllerDelegate {
        let onCode: @MainActor (String) -> Void
        init(onCode: @escaping @MainActor (String) -> Void) { self.onCode = onCode }

        func dataScanner(_ scanner: DataScannerViewController, didAdd addedItems: [RecognizedItem], allItems: [RecognizedItem]) {
            for item in addedItems {
                if case .barcode(let code) = item, let payload = code.payloadStringValue {
                    scanner.stopScanning()
                    onCode(payload)
                    return
                }
            }
        }
    }
}
#endif
