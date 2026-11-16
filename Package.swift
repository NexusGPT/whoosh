// swift-tools-version: 6.0
// One package, one UI target over Sources/ and one test target over Tests/ (repo-shape.md):
// a lane adds a file under its folder and it builds; nobody edits this file to add one.
import PackageDescription

let package = Package(
    name: "Whoosh",
    // String Catalogs (settings lane, 2026-11-07): English is the source language.
    defaultLocalization: "en",
    platforms: [.macOS(.v15), .iOS(.v18)],
    products: [
        .library(name: "WhooshUI", targets: ["WhooshUI"]),
    ],
    // Proven by the merged lanes (docs/lanes/embedding.md, docs/lanes/ondeviceai.md), 2026-11-07.
    // swift-transformers: embedding asked from 1.3.0, ondeviceai exact 1.4.0; exact 1.4.0 satisfies both.
    dependencies: [
        .package(url: "https://github.com/ml-explore/mlx-swift", exact: "0.33.0"),                                  // MIT
        .package(url: "https://github.com/ml-explore/mlx-swift-lm", revision: "4ab267996c83a1ae60789850cddfe646043cc36d"), // MIT
        .package(url: "https://github.com/huggingface/swift-transformers", exact: "1.4.0"),                         // Apache-2.0
        // docs/lanes/shortcuts.md "## Contract deltas" item 1 (MIT); macOS only, so linked on the Mac only.
        .package(url: "https://github.com/sindresorhus/KeyboardShortcuts", from: "2.0.0"),
        // docs/lanes/dictation.md item 4: the iPhone dictation engine (FluidAudio, Apache-2.0).
        .package(url: "https://github.com/FluidInference/FluidAudio.git", from: "0.18.2"),
    ],
    targets: [
        // libwhoosh_ffi.a for every platform slice, with the generated C header module `contractsFFI`.
        // Built by scripts/gen-swift.sh (scripts/build.sh runs it first); never committed.
        .binaryTarget(name: "contractsFFI", path: "core/target/WhooshFFI.xcframework"),
        .target(
            name: "WhooshUI",
            dependencies: [
                "contractsFFI",
                .product(name: "MLX", package: "mlx-swift"),
                .product(name: "MLXNN", package: "mlx-swift"),
                .product(name: "MLXFast", package: "mlx-swift"),
                .product(name: "MLXEmbedders", package: "mlx-swift-lm"),
                .product(name: "MLXVLM", package: "mlx-swift-lm"),
                .product(name: "MLXLMCommon", package: "mlx-swift-lm"),
                .product(name: "MLXLLM", package: "mlx-swift-lm"),
                .product(name: "MLXHuggingFace", package: "mlx-swift-lm"),
                .product(name: "Tokenizers", package: "swift-transformers"),
                .product(name: "KeyboardShortcuts", package: "KeyboardShortcuts", condition: .when(platforms: [.macOS])),
                .product(name: "FluidAudio", package: "FluidAudio"),
            ],
            path: "Sources",
            // The one String Catalog (i18n lane, 14 languages): Sources/Resources/Localizable.xcstrings. SwiftPM finds
            // no .xcstrings on its own (probed 2026-11-07), so it is declared here; features read it through Words.text.
            resources: [.process("Resources")],
            // The real core (core/crates/engine) reads the system's DNS and proxy settings (hickory, reqwest): the Rust
            // library needs SystemConfiguration, on the Mac and on iOS (real core slice 1, 2026-11-09: undefined _SC* at link).
            linkerSettings: [.linkedFramework("SystemConfiguration")]
        ),
        .testTarget(
            name: "WhooshTests",
            dependencies: ["WhooshUI"],
            path: "Tests"
        ),
    ]
)
