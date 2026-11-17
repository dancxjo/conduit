use super::*;
#[test]
fn owned_capacity_includes_spare_storage_and_nested_boxes() {
    let mut text = String::with_capacity(257);
    text.push('x');
    let mut values = Vec::with_capacity(7);
    values.push(Box::new(text));
    assert_eq!(
        values.backing(),
        Some(
            values.capacity() * size_of::<Box<String>>()
                + size_of::<String>()
                + values[0].capacity()
        )
    );
}
#[test]
fn checked_sum_refuses_overflow() {
    assert_eq!(sum([Some(usize::MAX), Some(1)]), None);
    assert_eq!(sum([Some(1), None]), None);
}
#[test]
fn tree_envelope_includes_entry_payload_capacity() {
    let mut tree = BTreeMap::new();
    let mut key = String::with_capacity(400);
    key.push_str("key");
    tree.insert(key, String::from("value"));
    let before = tree.backing().unwrap();
    tree.values_mut().next().unwrap().reserve(1000);
    assert!(tree.backing().unwrap() >= before + 990);
}
