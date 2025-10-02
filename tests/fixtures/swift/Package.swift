// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "MySwiftApp",
    platforms: [
        .macOS(.v13),
        .iOS(.v16)
    ],
    products: [
        .library(
            name: "MySwiftApp",
            targets: ["MySwiftApp"]),
    ],
    dependencies: [
        .package(url: "https://github.com/vapor/vapor.git", from: "4.89.0"),
        .package(url: "https://github.com/vapor/fluent.git", from: "4.8.0"),
        .package(url: "https://github.com/apple/swift-nio.git", from: "2.61.0"),
        .package(name: "Alamofire", url: "https://github.com/Alamofire/Alamofire.git", from: "5.8.1"),
    ],
    targets: [
        .target(
            name: "MySwiftApp",
            dependencies: [
                .product(name: "Vapor", package: "vapor"),
                .product(name: "Fluent", package: "fluent"),
                "Alamofire"
            ]),
        .testTarget(
            name: "MySwiftAppTests",
            dependencies: ["MySwiftApp"]),
    ]
)
