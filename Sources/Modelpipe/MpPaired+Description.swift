// Hand-written, beside the generated binding and never inside it. A UniFFI
// record is a plain Swift struct, so `print(paired)` or `String(describing:)`
// would render every field, the device key included; these two renderings
// say what a log line needs and withhold the key. They do not cover `Mirror`,
// `dump()` or a direct read of `apiKey`: the key is a plain `String` the app
// owns from the moment `mpPair` returns. The smoke test pins both renderings.
extension MpPaired: CustomStringConvertible, CustomDebugStringConvertible {
    public var description: String {
        "MpPaired(device: \(device), serving: \(serving), apiKey: <redacted>)"
    }

    public var debugDescription: String { description }
}
