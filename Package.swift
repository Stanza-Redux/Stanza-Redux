// swift-tools-version: 5.9
// This is a Skip (https://skip.tools) package,
// containing a Swift Package Manager project
// that will use the Skip build plugin to transpile the
// Swift Package, Sources, and Tests into an
// Android Gradle Project with Kotlin sources and JUnit tests.
import PackageDescription

let package = Package(
    name: "stanza-app",
    defaultLocalization: "en",
    platforms: [.iOS(.v17), .macOS(.v14), .tvOS(.v17), .watchOS(.v10), .macCatalyst(.v17)],
    products: [
        .library(name: "StanzaApp", type: .dynamic, targets: ["Stanza"]),
        .library(name: "StanzaModel", type: .dynamic, targets: ["StanzaModel"]),
    ],
    dependencies: [
        .package(url: "https://source.skip.tools/skip.git", from: "1.1.3"),
        .package(url: "https://github.com/appfair/appfair-app.git", from: "1.0.0"),
        .package(url: "https://source.skip.tools/skip-model.git", from: "1.0.0"),
        .package(url: "https://source.skip.tools/skip-ui.git", from: "1.0.0"),
        .package(url: "https://source.skip.tools/skip-sql.git", "0.0.0"..<"2.0.0"),
        .package(url: "https://source.skip.tools/skip-kit.git", "0.0.0"..<"2.0.0"),
        .package(url: "https://github.com/readium/swift-toolkit.git", from: "3.7.0"),
    ],
    targets: [
        .target(name: "Stanza", dependencies: [
            "StanzaModel",
            .product(name: "AppFairUI", package: "appfair-app"),
            .product(name: "SkipUI", package: "skip-ui"),
            .product(name: "SkipKit", package: "skip-kit"),
            .product(name: "ReadiumNavigator", package: "swift-toolkit", condition: .when(platforms: [.iOS])),
        ], resources: [.process("Resources")], plugins: [.plugin(name: "skipstone", package: "skip")]),
        .testTarget(name: "StanzaTests", dependencies: [
            "Stanza",
            .product(name: "SkipTest", package: "skip")
        ], resources: [.process("Resources")], plugins: [.plugin(name: "skipstone", package: "skip")]),

        .target(name: "StanzaModel", dependencies: [
            .product(name: "SkipModel", package: "skip-model"),
            .product(name: "SkipSQL", package: "skip-sql"),
            .product(name: "ReadiumShared", package: "swift-toolkit"),
            .product(name: "ReadiumOPDS", package: "swift-toolkit"),
            //.product(name: "ReadiumLCP", package: "swift-toolkit"),
            .product(name: "ReadiumStreamer", package: "swift-toolkit"),
            .product(name: "ReadiumAdapterGCDWebServer", package: "swift-toolkit"),
        ], resources: [.process("Resources")], plugins: [.plugin(name: "skipstone", package: "skip")]),
        .testTarget(name: "StanzaModelTests", dependencies: [
            "StanzaModel",
            .product(name: "SkipTest", package: "skip")
        ], resources: [.process("Resources")], plugins: [.plugin(name: "skipstone", package: "skip")]),
    ]
)
