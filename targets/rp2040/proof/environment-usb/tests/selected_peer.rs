//! Pure address/observed-attachment policy; no SDK socket or radio proof.
#[path = "../src/wifi_peer.rs"]
mod peer;
use peer::*;
#[test]
fn only_explicit_private_on_link_unicast_is_selected() {
    assert!(Peer::select([10, 0, 0, 12], [10, 0, 0, 181], 24, 1).is_ok());
    for address in [
        [8, 8, 8, 8],
        [127, 0, 0, 1],
        [224, 0, 0, 1],
        [255, 255, 255, 255],
        [10, 0, 1, 12],
        [10, 0, 0, 0],
        [10, 0, 0, 255],
        [10, 0, 0, 181],
    ] {
        assert!(
            Peer::select(address, [10, 0, 0, 181], 24, 1).is_err(),
            "{address:?}"
        );
    }
    for prefix in [0, 31, 32, 33] {
        assert!(Peer::select([10, 0, 0, 12], [10, 0, 0, 181], prefix, 1).is_err());
    }
    assert!(Peer::select([10, 0, 0, 12], [10, 0, 0, 181], 24, 0).is_err());
}
#[test]
fn link_loss_rejoin_and_local_configuration_change_invalidate_selection() {
    let mut a = Attachment::default();
    assert!(a.select([10, 0, 0, 12]).is_err());
    a.observe(Some(([10, 0, 0, 181], 24))).unwrap();
    a.select([10, 0, 0, 12]).unwrap();
    assert!(a.ready());
    let generation = a.generation;
    let old = a.peer.unwrap();
    a.observe(Some(([10, 0, 0, 181], 24))).unwrap();
    assert_eq!(a.generation, generation);
    assert!(a.ready());
    a.observe(None).unwrap();
    assert!(!a.ready());
    assert!(a.generation > generation);
    a.observe(Some(([10, 0, 0, 181], 24))).unwrap();
    assert!(!a.ready());
    assert!(!old.matches([10, 0, 0, 181], 24, a.generation));
    a.select([10, 0, 0, 12]).unwrap();
    a.observe(Some(([10, 0, 0, 182], 24))).unwrap();
    assert!(!a.ready());
    a.select([10, 0, 0, 12]).unwrap();
    a.invalidate().unwrap();
    assert!(!a.ready());
}
#[test]
fn generation_exhaustion_fences_peer_without_wrapping() {
    let mut a = Attachment::default();
    a.observe(Some(([192, 168, 1, 4], 24))).unwrap();
    a.select([192, 168, 1, 5]).unwrap();
    a.generation = u64::MAX;
    assert!(a.invalidate().is_err());
    assert_eq!(a.generation, u64::MAX);
    assert!(!a.ready());
}
