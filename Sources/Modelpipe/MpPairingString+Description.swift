// Hand-written, beside the generated binding and never inside it, for the
// reason `MpPaired+Description.swift` exists: a UniFFI record is a plain
// Swift struct, so `print(read)` would render the ticket in full, and a
// ticket is what this library redacts wherever an error renders one. Whether
// there is a code is what a log line needs. `Mirror`, `dump()` and a direct
// read of `ticket` are not covered; the field is the app's to show.
extension MpPairingString: CustomStringConvertible, CustomDebugStringConvertible {
    public var description: String {
        "MpPairingString(ticket: <redacted>, hasCode: \(hasCode))"
    }

    public var debugDescription: String { description }
}
