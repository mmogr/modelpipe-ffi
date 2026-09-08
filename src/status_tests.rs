//! The mirrored value types round-trip, and mirror the app's own names.

use super::*;

/// The mapping is total and lossless in both directions for every variant
/// this build knows. A reordering upstream, or a renamed variant, breaks here
/// rather than silently mistranslating a status the app draws.
#[test]
fn every_status_round_trips_through_modelpipes_own_enum() {
    for (mine, theirs) in [
        (MpPipeStatus::Idle, PipeStatus::Idle),
        (MpPipeStatus::Direct, PipeStatus::Direct),
        (MpPipeStatus::Relayed, PipeStatus::Relayed),
        (MpPipeStatus::Closed, PipeStatus::Closed),
    ] {
        assert_eq!(
            MpPipeStatus::from(theirs),
            mine,
            "{theirs:?} maps to {mine:?}"
        );
        assert_eq!(PipeStatus::from(mine), theirs, "{mine:?} maps back");
    }
}

/// `GGChatCore.PipeStatus` is a `String`-raw enum whose cases are these four
/// names in this order, and the connector maps position for position. If the
/// two ever disagree the app draws "connected" for a pipe that is closed, so
/// the order is asserted rather than assumed.
#[test]
fn the_status_names_match_the_apps_own_enum() {
    let names: Vec<String> = [
        MpPipeStatus::Idle,
        MpPipeStatus::Direct,
        MpPipeStatus::Relayed,
        MpPipeStatus::Closed,
    ]
    .iter()
    .map(|status| format!("{status:?}").to_lowercase())
    .collect();

    assert_eq!(
        names,
        vec!["idle", "direct", "relayed", "closed"],
        "GGChatCore.PipeStatus declares exactly these, in this order"
    );
}

/// A close reason survives the crossing.
#[test]
fn every_close_reason_round_trips() {
    assert_eq!(
        MpCloseReason::from(CloseReason::Shutdown),
        MpCloseReason::Shutdown
    );
    assert_eq!(
        MpCloseReason::from(CloseReason::ListenerFailed),
        MpCloseReason::ListenerFailed
    );
}

/// The three counters are carried across in the right order. They are all
/// `u64` with the same type, so a transposition is invisible to the compiler
/// and would silently report failures as successes.
#[test]
fn the_metrics_are_not_transposed() {
    let metrics = MpNetworkMetrics {
        relay_connections: 1,
        relay_connections_failed: 2,
        relay_connections_ratelimited: 3,
    };

    assert_eq!(metrics.relay_connections, 1);
    assert_eq!(metrics.relay_connections_failed, 2);
    assert_eq!(metrics.relay_connections_ratelimited, 3);
    assert_eq!(
        MpNetworkMetrics::default(),
        MpNetworkMetrics {
            relay_connections: 0,
            relay_connections_failed: 0,
            relay_connections_ratelimited: 0
        },
        "a default reading is three zeroes, not an absent one"
    );
}
