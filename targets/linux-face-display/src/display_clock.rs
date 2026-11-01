//! Local Mask status only: never evidence that a producer or measurement is live.
const HEIGHT: u16 = 32;

pub fn content_height(height: u16) -> Result<u16, String> {
    height
        .checked_sub(HEIGHT)
        .filter(|h| *h > 36)
        .ok_or_else(|| "display too short for content and local clock".into())
}

fn utc_time(seconds: libc::time_t) -> Option<String> {
    // Refuse pre-epoch and dates outside a four-digit year. No claim of NTP sync.
    if seconds < 0 {
        return None;
    }
    let mut calendar = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: gmtime_r initializes the supplied tm on success; neither pointer escapes.
    let result = unsafe { libc::gmtime_r(&seconds, calendar.as_mut_ptr()) };
    if result.is_null() {
        return None;
    }
    // SAFETY: non-null gmtime_r result proves the tm was initialized.
    let calendar = unsafe { calendar.assume_init() };
    let year = calendar.tm_year.checked_add(1900)?;
    if !(1970..=9999).contains(&year) {
        return None;
    }
    Some(format!(
        "{:02}:{:02}:{:02} UTC",
        calendar.tm_hour, calendar.tm_min, calendar.tm_sec
    ))
}

fn label(time: Option<&str>, elapsed_seconds: u64, width: u16) -> String {
    let pulse = if elapsed_seconds.is_multiple_of(2) {
        '+'
    } else {
        'o'
    };
    if width < 600 {
        return format!("{} DISPLAY {pulse}", time.unwrap_or("NO CLOCK"));
    }
    format!(
        "{} | DISPLAY {pulse} | host clock",
        time.unwrap_or("CLOCK UNAVAILABLE")
    )
}

pub fn render(
    width: u16,
    height: u16,
    elapsed_seconds: u64,
    pixels: &mut [u32],
) -> Result<(), String> {
    let content = content_height(height)?;
    if pixels.len() != usize::from(width) * usize::from(height) {
        return Err("clock raster extent mismatch".into());
    }
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: valid writable timespec; failure is displayed without stopping the Mask.
    let time = if unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut now) } == 0 {
        utc_time(now.tv_sec)
    } else {
        None
    };
    let footer = &mut pixels[usize::from(content) * usize::from(width)..];
    footer.fill(0x242a32);
    crate::view::render_notice(
        &label(time.as_deref(), elapsed_seconds, width),
        width,
        HEIGHT,
        footer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utc_rollover_and_invalid_time_are_explicit() {
        assert_eq!(utc_time(0).as_deref(), Some("00:00:00 UTC"));
        assert_eq!(utc_time(86_399).as_deref(), Some("23:59:59 UTC"));
        assert_eq!(utc_time(86_400).as_deref(), Some("00:00:00 UTC"));
        assert!(utc_time(-1).is_none());
        assert!(label(None, 0, 1024).contains("CLOCK UNAVAILABLE"));
        assert_ne!(label(None, 0, 1024), label(None, 1, 1024));
        assert_eq!(label(None, 0, 1024), label(None, 2, 1024));
    }
    #[test]
    fn clock_labels_fit_the_reserved_line_at_display_sizes() {
        use conduitos::display::typography::{TextLayout, TextRole};
        for width in [240, 320, 800, 1024] {
            for text in [label(Some("23:59:59 UTC"), 0, width), label(None, 1, width)] {
                let height = TextLayout::new(&text, TextRole::Body, width - 16)
                    .unwrap()
                    .finish_height();
                assert!(
                    height <= 24,
                    "clock label wraps beyond the footer: {height}"
                );
            }
        }
    }
    #[test]
    fn footer_reserves_space_and_cannot_overwrite_content() {
        assert_eq!(content_height(600).unwrap(), 568);
        assert!(content_height(68).is_err());
        let mut pixels = vec![0x123456; 1024 * 600];
        render(1024, 600, 0, &mut pixels).unwrap();
        assert!(pixels[..1024 * 568].iter().all(|p| *p == 0x123456));
        assert!(pixels[1024 * 568..].iter().any(|p| *p != 0x123456));
        assert!(render(1024, 600, 0, &mut []).is_err());
    }
}
