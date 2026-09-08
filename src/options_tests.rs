//! The options carry across, and the defaults are upstream's defaults.

use super::*;

/// The default this crate hands out has to be the default modelpipe would
/// have used. They are written in two places — a `Default` impl here and
/// upstream's — and a drift between them is a behaviour change nobody sees.
#[test]
fn the_defaults_match_modelpipes_own() {
    let mine = MpConnectOptions::default();
    let applied = mine.apply();
    let theirs = ConnectOptions::default();

    assert_eq!(applied.bind, theirs.bind, "bind");
    assert_eq!(applied.relay, theirs.relay, "relay");
    assert_eq!(
        applied.port_mapping, theirs.port_mapping,
        "port mapping is on by default upstream"
    );
    assert_eq!(
        applied.discovery, theirs.discovery,
        "discovery is on by default upstream"
    );
    assert_eq!(applied.relay_only, theirs.relay_only, "relay_only");
}

/// A port asked for is a loopback port. Binding a requested port on any other
/// interface would put the pipe's listener on the network, which is the one
/// thing the loopback-at-the-seam design exists to prevent.
#[test]
fn a_requested_port_binds_loopback_and_only_loopback() {
    let applied = MpConnectOptions {
        port: Some(8080),
        ..MpConnectOptions::default()
    }
    .apply();

    let bind = applied.bind.expect("a port was asked for");
    assert_eq!(bind.port(), 8080);
    assert!(
        bind.ip().is_loopback(),
        "a requested port must not reach the network: {bind}"
    );
}

/// No port means no bind, which is what lets the OS pick a free one. Asserting
/// it because `Some(0)` and `None` are easy to conflate and behave differently
/// on a phone, where a fixed port is a way to fail a dial that would work.
#[test]
fn no_port_asked_for_is_no_bind_at_all() {
    assert!(MpConnectOptions::default().apply().bind.is_none());
}

/// Every switch reaches upstream, set to the opposite of its default so a
/// field that silently never got copied fails here.
#[test]
fn every_switch_is_carried_across() {
    let applied = MpConnectOptions {
        port: None,
        relay_url: Some("https://relay.example".to_owned()),
        port_mapping: false,
        discovery: false,
        relay_only: true,
    }
    .apply();

    assert_eq!(applied.relay.as_deref(), Some("https://relay.example"));
    assert!(!applied.port_mapping);
    assert!(!applied.discovery);
    assert!(applied.relay_only);
}
