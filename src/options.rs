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
}

impl Default for MpConnectOptions {
    fn default() -> Self {
        Self {
            port: None,
            relay_url: None,
            port_mapping: true,
            discovery: true,
            relay_only: false,
        }
    }
}

impl MpConnectOptions {
    /// Build modelpipe's options from these.
    ///
    /// `default()`-then-assign because `ConnectOptions` is `#[non_exhaustive]`
    /// and a struct literal will not compile from outside its crate. That is
    /// upstream working as intended: a field added there arrives here as a
    /// default rather than as a build failure.
    pub(crate) fn apply(&self) -> ConnectOptions {
        let mut opts = ConnectOptions::default();
        opts.bind = self
            .port
            .map(|port| std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)));
        opts.relay.clone_from(&self.relay_url);
        opts.port_mapping = self.port_mapping;
        opts.discovery = self.discovery;
        opts.relay_only = self.relay_only;
        opts
    }
}

#[cfg(test)]
#[path = "options_tests.rs"]
mod options_tests;
