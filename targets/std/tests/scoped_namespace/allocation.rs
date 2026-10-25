use super::*;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn prepared_list_open_and_refusal_do_not_allocate() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("input"), b"input").unwrap();
    let mut owner = provider(temp.path(), 1);
    let (mut list, list_claim) = issue(&mut owner, LIST_CALL);
    let entry = owner.list(&mut list, &list_claim).unwrap()[0]
        .entry_id
        .clone();
    let (mut read, read_claim) = issue(&mut owner, OPEN_CALL);
    let mut output = [0u8; 256];
    let (_, observation) = allocation_probe::observe(|| {
        assert_eq!(owner.list(&mut list, &list_claim).unwrap().len(), 1);
        assert_eq!(
            owner
                .open_read(&mut read, &read_claim, &entry, &mut output)
                .unwrap(),
            5
        );
        owner.revoke_play(&read_claim.active_play_id);
        assert!(owner
            .open_read(&mut read, &read_claim, &entry, &mut output)
            .is_err());
    });
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
}

#[test]
fn refusal_scrubs_only_admitted_prefix_even_with_oversized_caller_storage() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("input"), b"oversized").unwrap();
    let mut small = limits();
    small.maximum_read_bytes = 2;
    let mut owner = ScopedNamespace::prepare(
        File::open(temp.path()).unwrap(),
        "host".into(),
        "boot".into(),
        binding(),
        "root".into(),
        ResourceGenerationId("root/1".into()),
        small,
        [1; 32],
    )
    .unwrap();
    let (mut list, c) = issue(&mut owner, LIST_CALL);
    let entry = owner.list(&mut list, &c).unwrap()[0].entry_id.clone();
    let (mut read, c) = issue(&mut owner, OPEN_CALL);
    let mut output = [9; 512];
    assert!(matches!(
        owner.open_read(&mut read, &c, &entry, &mut output),
        Err(NamespaceRefusal::ByteBound)
    ));
    assert_eq!(&output[..2], &[0; 2]);
    assert!(output[2..].iter().all(|byte| *byte == 9));
}

#[test]
fn name_and_descriptor_bounds_refuse_before_publication() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("long-name"), b"sentinel").unwrap();
    let mut names = limits();
    names.maximum_name_bytes = 1;
    let mut handles = limits();
    handles.maximum_handles = 1;
    for bounds in [names, handles] {
        assert!(matches!(
            ScopedNamespace::prepare(
                File::open(temp.path()).unwrap(),
                "host".into(),
                "boot".into(),
                binding(),
                "root".into(),
                ResourceGenerationId("root/1".into()),
                bounds,
                [1; 32]
            ),
            Err(NamespaceRefusal::NameBound | NamespaceRefusal::InvalidBounds)
        ));
    }
    assert_eq!(
        fs::read(temp.path().join("long-name")).unwrap(),
        b"sentinel"
    );
}
