// swift-tools-version: 6.0
// Local package for the iPhone Notification Service Extension lane. The root Package.swift is frozen,
// so the extension's logic is proved here until the root wires the target into apps/ios/project.yml
// (see docs/lanes/notificationservice.md, "Contract deltas").
import PackageDescription

let package = Package(
    name: "WhooshNotificationService",
    platforms: [.macOS(.v15), .iOS(.v18)],
    products: [
        .library(name: "NotificationServiceKit", targets: ["NotificationServiceKit"]),
    ],
    targets: [
        // One flat target: the folder root holds the extension's sources and Info.plist exactly as the Xcode target reads them.
        .target(name: "NotificationServiceKit", path: ".", exclude: ["Info.plist", "Tests", ".build"],
                sources: ["CompletionGate.swift", "ExtensionWiring.swift", "GraphRichDecryptor.swift", "Lookup.swift",
                          "NotificationActionRouter.swift", "NotificationCategories.swift", "NotificationService.swift",
                          "PushPayload.swift", "ServiceRunner.swift"]),
        .testTarget(name: "NotificationServiceKitTests", dependencies: ["NotificationServiceKit"],
                    path: "Tests/NotificationServiceKitTests"),
    ]
)
