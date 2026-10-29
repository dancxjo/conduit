//! Exact legacy identity transcripts without an allocated formatting buffer.
use alloc::{collections::TryReserveError, string::String, vec::Vec};
use sha2::{Digest, Sha256};

fn digest_fields(prefix: &str, fields: &[&str]) -> Result<String, TryReserveError> {
    let mut hash = Sha256::new();
    hash.update(prefix.as_bytes());
    for field in fields {
        hash.update(b":");
        hash.update(field.as_bytes());
    }
    let digest = hash.finalize();
    let mut encoded = String::new();
    encoded.try_reserve_exact(64)?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 15)] as char);
    }
    Ok(encoded)
}
pub(crate) fn placement(
    plot: &str,
    gear: &str,
    host: &str,
    capability: &str,
) -> Result<conduit_core::PlacementId, TryReserveError> {
    digest_fields("placement", &[plot, gear, host, capability]).map(Into::into)
}
pub(crate) fn connection(
    plot: &str,
    connection: conduit_plot::CheckedConnectionRef<'_>,
) -> Result<conduit_core::ConnectionId, TryReserveError> {
    digest_fields(
        "connection",
        &[
            plot,
            connection.source_gear_id,
            connection.source_port_id,
            connection.sink_gear_id,
            connection.sink_port_id,
            connection.value_kind,
            connection.track.as_str(),
            connection.temporal.as_str(),
        ],
    )
    .map(Into::into)
}
/// All identifiers outlive their borrowed Plan rows. Refusal drops every
/// already-created identifier and never publishes a partial collection.
pub(crate) fn connections(
    plot: &str,
    source: &[conduit_plot::CheckedConnectionRef<'_>],
) -> Result<Vec<conduit_core::ConnectionId>, TryReserveError> {
    let mut ids = Vec::new();
    ids.try_reserve_exact(source.len())?;
    for item in source {
        ids.push(connection(plot, *item)?);
    }
    Ok(ids)
}
