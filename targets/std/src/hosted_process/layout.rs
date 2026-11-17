//! Private producer recipe, valid only for the reviewed fixed LC_ALL=C Command.
use std::{
    ffi::OsString,
    mem::{size_of, MaybeUninit},
    ptr::NonNull,
};
/// Same field types as qualified alloc::collections::btree::node::LeafNode.
#[allow(dead_code)]
struct EnvironmentLeaf<K, V> {
    parent: Option<NonNull<()>>,
    parent_idx: MaybeUninit<u16>,
    len: u16,
    keys: [MaybeUninit<K>; 11],
    vals: [MaybeUninit<V>; 11],
}
pub(crate) struct ProcessLayout {
    pub(crate) preparation_bytes: usize,
    pub(crate) spawn_bytes: usize,
    pub(crate) retained_bytes: usize,
}
pub(crate) const DIAGNOSTIC_BYTES: usize = 512;
/// Sum named producer requests, including all Vec growth allocations. This
/// bounds coexistence without guessing a multiplier for private Command data.
pub(crate) fn process_layout(
    program_bytes: usize,
    arguments: impl IntoIterator<Item = usize>,
    stdout: usize,
    stderr: usize,
) -> Result<ProcessLayout, &'static str> {
    if !cfg!(conduit_prepared_process_layout) {
        return Err(env!("CONDUIT_PREPARED_PROCESS_LAYOUT"));
    }
    let leaf = size_of::<EnvironmentLeaf<OsString, OsString>>();
    let configured_leaf = size_of::<EnvironmentLeaf<OsString, Option<OsString>>>();
    let mut preparation = program_bytes
        .checked_add(1)
        .and_then(|n| n.checked_mul(2))
        .ok_or("process layout overflow")?;
    let mut add = |n: usize| -> Result<(), &'static str> {
        preparation = preparation
            .checked_add(n)
            .ok_or("process layout overflow")?;
        Ok(())
    };
    let mut pointers = 2usize;
    add(pointers * size_of::<usize>())?;
    // argv0 + null already occupy two pointer entries.
    let mut entries = 2usize;
    for length in arguments {
        add(length.checked_add(1).ok_or("process argument overflow")?)?;
        entries = entries
            .checked_add(1)
            .ok_or("process argument count overflow")?;
        if entries > pointers {
            pointers = pointers.checked_mul(2).ok_or("process argv overflow")?;
            add(pointers
                .checked_mul(size_of::<usize>())
                .ok_or("process argv overflow")?)?;
        }
    }
    add(configured_leaf)?;
    add(6 + 1)?; // configured LC_ALL key/value
    add(stdout)?;
    add(stderr)?;
    add(DIAGNOSTIC_BYTES)?;
    // capture_env clones one leaf/key/value; envp reserves two pointers and
    // extends the cloned six-byte key exactly to LC_ALL=C\0 (nine bytes).
    let retained_bytes = preparation
        - (2..pointers)
            .filter(|n| n.is_power_of_two())
            .map(|n| n * size_of::<usize>())
            .sum::<usize>();
    let spawn_bytes = leaf
        .checked_add(6 + 1 + 2 * size_of::<usize>() + 9)
        .ok_or("process env overflow")?;
    Ok(ProcessLayout {
        preparation_bytes: preparation,
        spawn_bytes,
        retained_bytes,
    })
}
