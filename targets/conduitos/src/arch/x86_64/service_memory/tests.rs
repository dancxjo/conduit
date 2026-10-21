use super::*;
use crate::service_memory::ServiceProfile;

#[test]
fn independent_profile_ownership_and_quarantine() {
    let mut compositor = AddressSpace {
        cr3: 1,
        entry: 0x400000,
        cleared: false,
        lease: Lease::claim(ServiceProfile::Compositor).unwrap(),
    };
    let mut draft = AddressSpace {
        cr3: 2,
        entry: 0x400000,
        cleared: false,
        lease: Lease::claim(ServiceProfile::Draft).unwrap(),
    };
    assert!(Lease::claim(ServiceProfile::Compositor).is_err());
    assert!(Lease::claim(ServiceProfile::Draft).is_err());
    assert_eq!(compositor.reserved_bytes(), 19_349_504);
    assert_eq!(draft.reserved_bytes(), 2_228_224);
    assert_eq!(compositor.transfer_window().unwrap().len(), 65536);
    assert_eq!(draft.transfer_window().unwrap().len(), 16384);
    assert_ne!(
        compositor.exchange().unwrap().as_ptr(),
        draft.exchange().unwrap().as_ptr()
    );
    compositor.exchange().unwrap()[0] = 17;
    draft.exchange().unwrap()[0] = 23;
    draft.transfer_window().unwrap()[16383] = 31;
    let trap = draft.context().unwrap().trap_stack;
    assert_ne!(compositor.context().unwrap().trap_stack, trap);
    assert_eq!(compositor.quarantine(), 19_349_504);
    assert_eq!(compositor.quarantine(), 0);
    assert!(compositor.exchange().is_err());
    assert!(compositor.output().is_err());
    assert!(compositor.context().is_err());
    assert!(Lease::claim(ServiceProfile::Compositor).is_err());
    assert_eq!(draft.exchange().unwrap()[0], 23);
    assert_eq!(draft.output().unwrap()[16383], 31);
    drop(compositor);
    let mut replacement = Lease::claim(ServiceProfile::Compositor).unwrap();
    assert!(replacement.view().exchange.iter().all(|byte| *byte == 0));
    assert_eq!(draft.context().unwrap().trap_stack, trap);
    drop(draft);
    let mut fresh = Lease::claim(ServiceProfile::Draft).unwrap();
    let mut view = fresh.view();
    assert!(view.exchange.iter().all(|byte| *byte == 0));
    assert!(view.output.iter().all(|byte| *byte == 0));
    assert!(view.pixels.is_empty() && view.scratch.is_empty());
    // Exercise the actual mapper without CR3 installation or privileged instructions.
    let layout = ServiceProfile::Draft.layout();
    for range in &layout.active_ranges()[1..] {
        map_private(&mut view, *range, 0x800000).unwrap();
        for address in (range.start()..range.end()).step_by(PAGE) {
            let entry = view.pt[(address >> 21) as usize].0[(address as usize >> 12) & 511];
            assert_eq!(entry & (NX | 7), NX | 7);
        }
        let guard = range.end();
        assert_eq!(
            view.pt[(guard >> 21) as usize].0[(guard as usize >> 12) & 511],
            0
        );
    }
    assert!(map_private(&mut view, layout.output(), 0x800000).is_err());
    assert!(map(&mut view, layout.end() + 0x200000, 4096, 0x800000, 7 | NX).is_err());
    assert_eq!(replacement.profile(), ServiceProfile::Compositor);
}
