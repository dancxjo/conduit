//! Collection stays first; custody progresses in bounded foreground operations.
use super::*;
use std::{net::SocketAddr, time::Instant};
#[path = "cadence.rs"]
mod cadence;
use cadence::Work;
pub(super) fn run_loop(
    collector: Collector,
    peers: Vec<&str>,
    directory: &Path,
    once: bool,
    custody: Option<(&Path, &Path)>,
    mut network: Option<network::Network>,
    projection: Option<(&Path, crate::observed_history::retained::Projection<'_>)>,
) -> Result<()> {
    let mut preferred = 0;
    let mut continuing = crate::observed_custody_live::Continuing::new();
    let mut backoff = Backoff::default();
    let mut connected = false;
    let mut address: Option<SocketAddr> = None;
    let mut collection_due = Instant::now();
    loop {
        // Check collection before EVERY custody operation, including retries.
        // Socket work is bounded separately; DNS/local verification/fsync are
        // synchronous and no hard real-time collection guarantee is claimed.
        let observed_due = custody.and_then(|_| continuing.due()).filter(|_| connected);
        let network_due = address
            .and_then(|_| network.as_ref().and_then(network::Network::due))
            .filter(|_| connected);
        let work = cadence::select(Instant::now(), collection_due, observed_due, network_due);
        if work == Work::Collect {
            let collected = collect_routes(&collector, &peers, preferred, directory);
            if let Some((binding, projection)) = &projection {
                // Failed collection never authorizes a different current view.
                if let Ok((route, _)) = &collected {
                    projection.after_attempt(
                        binding,
                        directory,
                        custody.map(|(_, path)| path),
                        peers[*route],
                    );
                } else if projection.producer_boot != "current" {
                    projection.after_attempt(binding, directory, None, peers[preferred]);
                }
            }
            match collected {
                Ok((route, epochs)) => {
                    if route != preferred {
                        eprintln!("recorder selected authenticated transport {}", peers[route]);
                    }
                    preferred = route;
                    if !connected {
                        eprintln!("recorder connected; {epochs} authenticated epochs available");
                    }
                    connected = true;
                    backoff.succeeded();
                    if once {
                        return Ok(());
                    }
                    // Cache transport selection outside the per-operation
                    // socket budget; signed Root/Body/Boot still gate custody.
                    address = peers[preferred]
                        .to_socket_addrs()
                        .ok()
                        .and_then(|mut addresses| addresses.find(SocketAddr::is_ipv4));
                    collection_due = Instant::now() + Duration::from_secs(30);
                }
                Err(error) => {
                    if once {
                        return Err(error);
                    }
                    if connected || backoff.failures == 0 {
                        eprintln!(
                            "recorder unavailable; retained readings remain on producer: {error}"
                        );
                    }
                    connected = false;
                    collection_due = Instant::now() + backoff.failed();
                }
            }
            continue;
        }
        if connected {
            // Observation custody runs before a simultaneously due journal
            // operation. Its own next time prevents starvation of the journal.
            if work == Work::Observed {
                if let Some((binding, continuity)) = custody {
                    continuing.tick(binding, continuity, peers[preferred], directory);
                }
                continue;
            }
            if work == Work::Network {
                if let (Some(network), Some(address)) = (&mut network, address) {
                    network.tick(peers[preferred], address);
                }
                continue;
            }
        }
        if let Work::Wait(wake) = work {
            std::thread::sleep(
                wake.saturating_duration_since(Instant::now())
                    .min(Duration::from_secs(30)),
            );
        }
    }
}
