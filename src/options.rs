//! What the caller may ask for on a dial.
//!
//! A record rather than a builder: `UniFFI` generates a Swift struct with a
//! memberwise initialiser and defaults, which reads better at the call site
//! than four setters. modelpipe's own `ConnectOptions` is `#[non_exhaustive]`
//! and therefore `default()`-then-assign only, which is what `MpConnectOptions::apply`
//! does.
//!
//! Note what is absent: **there is no token field, and there is none upstream
//! either.** `modelpipe::connect` takes no credential at all. The connecting
//! side binds a plain local HTTP listener and forwards `Authorization`
//! untouched; the serve edge is the only thing that checks it. So the bearer
//! token belongs to the HTTP client the app points at the base URL, and it is
//! deliberately impossible to hand one to this crate.

use std::path::Path;

use modelpipe::ConnectOptions;

/// Options for a dial. `MpConnectOptions::default()` is the right answer
/// almost always.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MpConnectOptions {
    /// Bind this loopback port instead of letting the OS pick a free one.
    ///
    /// Leave it `None`. A fixed port is for reproducing a bug, and on a phone
    /// it is a way to fail a dial that would otherwise have worked.
    #[uniffi(default = None)]
    pub port: Option<u16>,

    /// Use this relay instead of the public ones.
    #[uniffi(default = None)]
    pub relay_url: Option<String>,

    /// Ask the router to map a port (UPnP/NAT-PMP/PCP).
    ///
    /// On by default, as upstream. Harmless where the router says no.
    #[uniffi(default = true)]
    pub port_mapping: bool,

    /// Publish to, and resolve through, n0's discovery service.
    ///
    /// On by default. Turning it off restricts the dial to the addresses the
    /// ticket was minted with, which for a machine that has since changed
    /// network means no dial at all.
    #[uniffi(default = true)]
    pub discovery: bool,

    /// Skip hole punching and go through a relay from the start.
    ///
    /// Off by default. Useful only for making the relay path reproducible,
    /// which is otherwise hard to force.
    #[uniffi(default = false)]
    pub relay_only: bool,

    /// The directory this device keeps its endpoint keys in, so the far
    /// machine sees the same device every time. `None` mints a fresh key per
    /// process and writes nothing.
    ///
    /// A directory rather than a file, because the file is one per far
    /// machine — a relay allows a single live connection per endpoint id, so
    /// a device holding two machines has to meet each as a different device —
    /// and naming it takes the parsed ticket, which this record has never
    /// had. `crate::identity_file` has the rule and why it is a contract with
    /// the app rather than a detail of this crate.
    ///
    /// **The directory has to be there already.** Nothing here creates one:
    /// an app that wants its keys unreadable by others and out of its backups
    /// sets both as the directory is made, and neither survives being applied
    /// after the fact. A directory that is missing or cannot be written
    /// arrives as [`MpError::Identity`](crate::MpError::Identity), which
    /// names the file and is permanent.
    ///
    /// The file itself is modelpipe's: written readable only by this user and
    /// refused when others can read it, as its own serve side does; on
    /// Windows the directory is the only protection. Appended last: `UniFFI`
    /// emits the Swift memberwise initialiser in declaration order.
    #[uniffi(default = None)]
    pub identity_dir: Option<String>,
}

impl Default for MpConnectOptions {
    fn default() -> Self {
        Self {
            port: None,
            relay_url: None,
            port_mapping: true,
            discovery: true,
            relay_only: false,
            identity_dir: None,
        }
    }
}

impl MpConnectOptions {
    /// Build modelpipe's options from these, with `identity` as the file to
    /// keep this device's key in.
    ///
    /// The path is an argument rather than a field read here because naming
    /// the file takes the parsed ticket, and this record holds no ticket and
    /// should not start to: it is the same record for a dial and for a
    /// pairing, which arrive at their ticket by different routes. Passing
    /// `None` keeps no key, whatever `identity_dir` says, which is what makes
    /// `apply` worth testing on its own.
    ///
    /// `default()`-then-assign because `ConnectOptions` is `#[non_exhaustive]`
    /// and a struct literal will not compile from outside its crate. That is
    /// upstream working as intended: a field added there arrives here as a
    /// default rather than as a build failure.
    pub(crate) fn apply(&self, identity: Option<&Path>) -> ConnectOptions {
        let mut opts = ConnectOptions::default();
        opts.bind = self
            .port
            .map(|port| std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)));
        opts.relay.clone_from(&self.relay_url);
        opts.port_mapping = self.port_mapping;
        opts.discovery = self.discovery;
        opts.relay_only = self.relay_only;
        opts.identity = identity.map(Path::to_path_buf);
        opts
    }
}

#[cfg(test)]
#[path = "options_tests.rs"]
mod options_tests;
