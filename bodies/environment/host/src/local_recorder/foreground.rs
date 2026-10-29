//! Collection and custody share one foreground owner.
use super::*;
pub(super) fn run_loop(
    collector: Collector,
    peers: Vec<&str>,
    directory: &Path,
    once: bool,
    custody: Option<(&Path, &Path)>,
) -> Result<()> {
    let mut preferred = 0;
    let mut continuing = crate::observed_custody_live::Continuing::new();
    let mut backoff = Backoff::default();
    let mut connected = false;
    loop {
        match collect_routes(&collector, &peers, preferred, directory) {
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
                if let Some((binding, continuity)) = custody {
                    continuing.tick(binding, continuity, peers[preferred], directory);
                }
                std::thread::sleep(Duration::from_secs(30));
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
                std::thread::sleep(backoff.failed());
            }
        }
    }
}
