//! Actual retained factory ownership across a service-style worker handoff.
use super::encounter_memory::probe;
use super::*;

#[test]
fn retained_factory_keeps_original_counter_through_worker_move_and_return() {
    let (mut host, prepared, _files) = image_custody::initialized_factory();
    let (other, _other_prepared, _other_files) = image_custody::initialized_factory();
    let mut factory = RetainedCrecheFactory::from_image(&host, prepared).unwrap();
    assert!(factory.is_image());
    assert!(host
        .advertisement()
        .resources
        .contains(&factory.resource_offer().unwrap()));
    assert!(factory.verify_image_custody(&other).is_err());
    factory.verify_image_custody(&host).unwrap();
    {
        let borrowed = factory.borrow_mut().unwrap();
        borrowed.verify_image_custody(&host).unwrap();
    }
    assert!(!probe(&mut host, 62));
    let (mut host, factory) = std::thread::spawn(move || {
        factory.verify_image_custody(&host).unwrap();
        (host, factory)
    })
    .join()
    .unwrap();
    factory.verify_image_custody(&host).unwrap();
    assert!(!probe(&mut host, 62));
    drop(factory);
    assert!(probe(&mut host, 62));
}
