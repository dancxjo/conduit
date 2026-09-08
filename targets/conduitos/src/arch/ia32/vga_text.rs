//! Bounded legacy-PC text receipt for an attended physical boot.

const WIDTH: usize = 80;
const HEIGHT: usize = 25;
const CELLS: usize = WIDTH * HEIGHT;
const ATTRIBUTE: u16 = 0x1f00;

pub(super) fn present_boot_receipt(
    profile_id: &[u8],
    build_id: &[u8],
    image_id: &[u8],
    host_id: &[u8],
    boot_id: &[u8],
) {
    let mut cells = [ATTRIBUTE | u16::from(b' '); CELLS];
    render(&mut cells, 1, b"CONDUITOS IA-32 / LEGACY BIOS");
    render(&mut cells, 3, b"HELLO, CONDUITOS");
    render_pair(&mut cells, 5, b"PROFILE", profile_id);
    render_pair(&mut cells, 8, b"BUILD", build_id);
    render_pair(&mut cells, 11, b"IMAGE", image_id);
    render_pair(&mut cells, 14, b"HOST", host_id);
    render_pair(&mut cells, 17, b"BOOT", boot_id);
    render(&mut cells, 20, b"PRODUCT ENTRY READY");
    render(
        &mut cells,
        22,
        b"Retain this screen for the attended physical receipt.",
    );

    let vga = 0x000b_8000 as *mut u16;
    for (index, cell) in cells.into_iter().enumerate() {
        // SAFETY: this path is called only for the selected IA-32 legacy-BIOS
        // carrier. The conventional VGA text aperture is an attempted physical
        // presenter; human observation, not this write, establishes success.
        unsafe { vga.add(index).write_volatile(cell) };
    }
}

fn render_pair(cells: &mut [u16; CELLS], row: usize, label: &[u8], value: &[u8]) {
    render(cells, row, label);
    render(cells, row + 1, value);
}

fn render(cells: &mut [u16; CELLS], row: usize, bytes: &[u8]) {
    if row >= HEIGHT {
        return;
    }
    for (column, byte) in bytes.iter().copied().take(WIDTH).enumerate() {
        let printable = if byte.is_ascii_graphic() || byte == b' ' {
            byte
        } else {
            b'?'
        };
        cells[row * WIDTH + column] = ATTRIBUTE | u16::from(printable);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_is_bounded_and_sanitizes_non_text() {
        let mut cells = [0u16; CELLS];
        render(&mut cells, 0, b"A\nB");
        assert_eq!(cells[0] as u8, b'A');
        assert_eq!(cells[1] as u8, b'?');
        assert_eq!(cells[2] as u8, b'B');

        let overlong = [b'X'; WIDTH + 1];
        render(&mut cells, 1, &overlong);
        assert_eq!(cells[WIDTH] as u8, b'X');
        assert_eq!(cells[2 * WIDTH - 1] as u8, b'X');
        assert_eq!(cells[2 * WIDTH], 0);
    }
}
