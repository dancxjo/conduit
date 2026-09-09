//! A bounded view of recent Presentation output, not retained authored State.
pub(super) const RESULT_WINDOW_BYTES: usize = 128;

pub(super) struct ResultWindow {
    bytes: [u8; RESULT_WINDOW_BYTES],
    len: usize,
    omitted: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum WindowRefusal {
    FragmentTooLarge,
    HistoryExhausted,
}

impl ResultWindow {
    pub(super) const fn new() -> Self {
        Self {
            bytes: [0; RESULT_WINDOW_BYTES],
            len: 0,
            omitted: 0,
        }
    }

    pub(super) fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len])
            .expect("only complete UTF-8 fragments enter the window")
    }

    pub(super) const fn omitted_bytes(&self) -> u64 {
        self.omitted
    }

    pub(super) fn append(&mut self, value: &str) -> Result<(), WindowRefusal> {
        if value.len() > RESULT_WINDOW_BYTES {
            return Err(WindowRefusal::FragmentTooLarge);
        }
        let mut remove = (self.len + value.len()).saturating_sub(RESULT_WINDOW_BYTES);
        while !self.as_str().is_char_boundary(remove) {
            remove += 1;
        }
        let omitted = self
            .omitted
            .checked_add(remove as u64)
            .ok_or(WindowRefusal::HistoryExhausted)?;
        self.bytes.copy_within(remove..self.len, 0);
        let retained = self.len - remove;
        self.bytes[retained..retained + value.len()].copy_from_slice(value.as_bytes());
        self.len = retained + value.len();
        self.omitted = omitted;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_output_stays_utf8_and_reports_exact_omission() {
        let mut window = ResultWindow::new();
        for _ in 0..1_000 {
            window.append("Æ").unwrap();
        }
        assert_eq!(window.as_str().len(), 128);
        assert_eq!(window.omitted_bytes(), 1_872);
        window.append("A").unwrap();
        assert_eq!(window.as_str().len(), 127);
        assert!(window.as_str().ends_with('A'));
        assert_eq!(window.omitted_bytes(), 1_874);
    }
}
