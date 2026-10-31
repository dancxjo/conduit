//! Actual Root projection with retained inert renderer; no GPIO or Body effects.
use super::*;

struct Display(ReadyRenderer<native::Provider>);
impl Display {
    fn append_to_advertisement(&self, host: &mut c::HostAdvertisement) -> Result<(), ()> {
        self.0.append_to_advertisement(host).map_err(|_| ())
    }
}
struct NativeRoot {
    host: c::HostId,
    boot: c::BootId,
    offer_generation: u64,
    display: Display,
}
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_display_native_offer.rs"]
mod projection;

#[test]
fn scoped_mask_offer_matches_discarded_catalogue_and_preserves_plan_and_refusals() {
    let pins = Rc::new(native::Pins::default());
    let ready = native::ready(pins.clone());
    let mut root = NativeRoot {
        host: ready.identity().host_id.clone(),
        boot: ready.identity().boot_id.clone(),
        offer_generation: 1,
        display: Display(ready),
    };
    let (scoped, scoped_memory) =
        allocation_probe::observe(|| root.display_native_advertisement().unwrap());
    // CPU comparison of the old construct/discard pattern. These unrelated
    // fixture descriptions are not claimed as current physical Root equipment.
    let (old, old_memory) = allocation_probe::observe(|| {
        let mut host = fixture::host();
        host.profile = "environment/esp32-native-root@1".into();
        // Full Root projection already constructed this renderer once before
        // the Mask discarded every offer and reconstructed it a second time.
        root.display.append_to_advertisement(&mut host).unwrap();
        host.capabilities = Vec::new();
        host.resources = Vec::new();
        host.bases = Vec::new();
        root.display.append_to_advertisement(&mut host).unwrap();
        host
    });
    assert_eq!(scoped, old);
    assert!(scoped_memory.peak_bytes < old_memory.peak_bytes);
    let software: Vec<_> = fixture::host()
        .capabilities
        .into_iter()
        .filter(|offer| offer.kind_id.as_str() != f::RENDERER_KIND)
        .collect();
    let mut hosts = [scoped, old];
    for host in &mut hosts {
        host.capabilities.extend(software.clone());
    }
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let grants = [native::grant(&root.display.0)];
    let run =
        |host: &c::HostAdvertisement, grants: &[c::AuthorityGrant]| source.plan(host, 8192, grants);
    assert_eq!(
        run(&hosts[0], &grants).unwrap(),
        run(&hosts[1], &grants).unwrap()
    );
    for variant in 0..3 {
        let mut changed = hosts.clone();
        for host in &mut changed {
            match variant {
                0 => host.resources.clear(),
                1 => host
                    .capabilities
                    .retain(|offer| offer.kind_id.as_str() != f::RENDERER_KIND),
                _ => (),
            }
        }
        let authority = if variant == 2 { &[][..] } else { &grants[..] };
        assert_eq!(
            format!("{:?}", run(&changed[0], authority).unwrap_err()),
            format!("{:?}", run(&changed[1], authority).unwrap_err())
        );
    }
    root.boot = "stale/boot".into();
    assert!(root.display_native_advertisement().is_err());
    assert_eq!(pins.edges.get(), 0);
    println!(
        "Mask native offer CPU scoped peak={} discarded-fixture peak={} retained={}",
        scoped_memory.peak_bytes,
        old_memory.peak_bytes,
        scoped_memory.retained_bytes()
    );
}
