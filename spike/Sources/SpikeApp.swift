import SwiftUI

/// The on-device spike.
///
/// Deliberately ugly and deliberately small. Its whole job is to put a real
/// pipe on a real phone and show what happens, so every design decision here
/// favours *visibility of the measurement* over anything else: state is a flat
/// list of strings, there is no architecture, and nothing is abstracted.
///
/// What it does not do: pairing. The desktop's key is pasted in by hand
/// (`gglib config settings show` prints `proxy_api_key` unmasked). Redeeming a
/// code is already proven by ggchat's own tests and would only add a second
/// thing that can fail while measuring the first.
@main
struct SpikeApp: App {
    var body: some Scene {
        WindowGroup { ContentView() }
    }
}

/// One line of the transcript, timestamped from the app's own start so the
/// intervals are readable without doing arithmetic.
struct Line: Identifiable {
    let id = UUID()
    let at: TimeInterval
    let text: String
}

@MainActor
final class Spike: ObservableObject {
    @Published var pairing = ""
    @Published var apiKey = ""
    @Published var lines: [Line] = []
    @Published var status = "—"
    @Published var dialling = false

    private var pipe: MpPipe?
    private var watcher: Task<Void, Never>?
    private let started = Date()

    func log(_ text: String) {
        lines.append(Line(at: Date().timeIntervalSince(started), text: text))
    }

    /// A pairing string is `<ticket>-<code>`; a bare ticket is also accepted.
    ///
    /// Split on the LAST hyphen, matching ggchat's rule — base32 never
    /// produces one, but being explicit costs nothing and a wrong split here
    /// would look like a bad ticket rather than a parsing bug.
    private var ticketOnly: String {
        let trimmed = pairing.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let hyphen = trimmed.lastIndex(of: "-") else { return trimmed }
        let suffix = trimmed[trimmed.index(after: hyphen)...]
        let isCode = suffix.count == 6 && suffix.allSatisfy(\.isNumber)
        return isCode ? String(trimmed[..<hyphen]) : trimmed
    }

    func dial() async {
        await hangUp()
        dialling = true
        defer { dialling = false }

        log("dialling…")
        let t0 = Date()
        do {
            // Discovery ON, port mapping ON — the defaults, and the point.
            // Turning either off would make this measure something easier
            // than the real thing.
            let opened = try await mpConnect(
                ticket: ticketOnly,
                options: MpConnectOptions()
            )
            pipe = opened
            let bound = Date().timeIntervalSince(t0)
            log(String(format: "connect returned in %.2fs — %@", bound, opened.baseUrl()))
            log("status \(opened.status()) (idle here is correct: the listener is up, the peer is not reached yet)")
            status = "\(opened.status())"
            watch(opened, from: t0)
        } catch let error as MpError {
            log("refused: \(error.localizedDescription)")
            log("retryable: \(error.isRetryable())")
        } catch {
            log("unexpected: \(error)")
        }
    }

    /// Follow the status to its end, timing the first transition away from
    /// `idle` — which is the number that answers "does it form at all, and how
    /// long does CGNAT take".
    private func watch(_ pipe: MpPipe, from t0: Date) {
        watcher?.cancel()
        watcher = Task { [weak self] in
            var held = pipe.status()
            var sawFirstChange = false
            while let next = await pipe.statusChangedSince(snapshot: held) {
                held = next
                guard let self else { return }
                let elapsed = Date().timeIntervalSince(t0)
                if !sawFirstChange {
                    sawFirstChange = true
                    await self.log(String(format: "FIRST PATH after %.2fs: %@", elapsed, "\(next)"))
                } else {
                    await self.log(String(format: "%.2fs: %@", elapsed, "\(next)"))
                }
                await MainActor.run { self.status = "\(next)" }
                await self.logMetrics(pipe)
            }
            await self?.log("status stream ended")
            if let reason = pipe.closeReason() {
                await self?.log("closed because: \(reason)")
            }
        }
    }

    private func logMetrics(_ pipe: MpPipe) async {
        let m = pipe.networkMetrics()
        // The rate-limit counter is the one nothing else can show: a relay
        // refusing this endpoint looks exactly like a network that will not
        // connect.
        log("relay: \(m.relayConnections) opened, \(m.relayConnectionsFailed) failed, \(m.relayConnectionsRatelimited) rate-limited")
    }

    /// The actual end-to-end proof: a real request over the pipe, answered by
    /// the machine at home.
    func fetchModels() async {
        guard let pipe else { log("dial first"); return }
        let key = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !key.isEmpty else { log("paste the desktop's proxy_api_key first"); return }
        guard let url = URL(string: pipe.baseUrl() + "/models") else { return }

        var request = URLRequest(url: url)
        request.setValue("Bearer \(key)", forHTTPHeaderField: "Authorization")
        request.timeoutInterval = 30

        let t0 = Date()
        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            let code = (response as? HTTPURLResponse)?.statusCode ?? -1
            let ms = Int(Date().timeIntervalSince(t0) * 1000)
            log("GET /v1/models → \(code) in \(ms)ms, \(data.count) bytes")
            if let body = String(data: data.prefix(400), encoding: .utf8) {
                log(body)
            }
        } catch {
            log("request failed: \(error.localizedDescription)")
        }
    }

    /// Question 3: does the pipe survive a suspension? Background the app,
    /// wait, come back, and press this before anything else.
    func resumed() async {
        guard let pipe else { return }
        log("resume: telling the endpoint the network may have moved")
        await pipe.notifyNetworkChange()
        log("resume: status is \(pipe.status())")
        await logMetrics(pipe)
    }

    func hangUp() async {
        watcher?.cancel()
        watcher = nil
        if let pipe {
            await pipe.shutdown()
            log("shut down; status \(pipe.status())")
        }
        pipe = nil
        status = "—"
    }
}

struct ContentView: View {
    @StateObject private var spike = Spike()
    @Environment(\.scenePhase) private var phase

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("status: \(spike.status)")
                .font(.headline.monospaced())

            TextField("pairing string or ticket", text: $spike.pairing)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)

            TextField("proxy_api_key", text: $spike.apiKey)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)

            HStack {
                Button("Dial") { Task { await spike.dial() } }
                    .disabled(spike.dialling || spike.pairing.isEmpty)
                Button("GET models") { Task { await spike.fetchModels() } }
                Button("Hang up") { Task { await spike.hangUp() } }
            }
            .buttonStyle(.borderedProminent)

            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(spike.lines) { line in
                        Text(String(format: "%6.2f  %@", line.at, line.text))
                            .font(.caption.monospaced())
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
        }
        .padding()
        .onChange(of: phase) { _, new in
            // The suspension question, asked automatically so it cannot be
            // forgotten: background the app, wait, and come back.
            if new == .active { Task { await spike.resumed() } }
        }
    }
}
