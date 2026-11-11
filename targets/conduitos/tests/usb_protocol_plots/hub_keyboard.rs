//! Boot keyboard selection must configure boot protocol and continue to touch.
use super::*;
#[test]
fn boot_keyboard_is_retained_before_touch_without_native_report_interpretation() {
    let mut tree = nested_hub_prefix();
    tree.push(exchange(0x0000000100080323, 2, 64, 0, &[]));
    tree.push(exchange(0x0000000200080323, 2, 64, 0, &[]));
    tree.push(exchange(0x00040001000000a3, 2, 64, 0, &[0, 1, 0, 0]));
    tree.push(exchange(0x00040002000000a3, 2, 64, 0, &[1, 3, 1, 0]));
    tree.push(exchange(0x0000000200040323, 2, 64, 0, &[]));
    tree.push(exchange(0x00040002000000a3, 2, 64, 0, &[3, 3, 16, 0]));
    tree.push(exchange(0x0000000200140123, 2, 64, 0, &[]));
    tree.push(exchange(0x0000000200100123, 2, 64, 0, &[]));
    for mut item in [
        exchange(0x0008000001000680, 0, 8, 2, &[18, 1, 0, 2, 0, 0, 0, 8]),
        exchange(0x00000000000a0500, 0, 8, 2, &[]),
    ] {
        item.translator_hub = 2;
        item.low_speed = true;
        tree.push(item);
    }
    let configuration = [
        9, 2, 34, 0, 1, 1, 0, 0x80, 50, 9, 4, 0, 0, 1, 3, 1, 1, 0, 9, 0x21, 0x11, 1, 0, 1, 0x22,
        55, 0, 7, 5, 0x81, 3, 8, 0, 10,
    ];
    for mut item in [
        exchange(0x0009000002000680, 10, 8, 2, &configuration[..9]),
        exchange(0x0022000002000680, 10, 8, 2, &configuration),
        exchange(0x0000000000010900, 10, 8, 2, &[]),
        exchange(0x0000000000000b21, 10, 8, 2, &[]),
    ] {
        item.translator_hub = 2;
        item.low_speed = true;
        tree.push(item);
    }
    tree.extend(fixture().into_iter().skip(9));
    run_fixture_with_keyboard(tree, &[2, 2, 50, 2, 2, 50, 2, 50, 2], 3, 1, Some(10));
}
