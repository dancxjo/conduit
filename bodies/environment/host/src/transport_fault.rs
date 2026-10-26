//! Bounded fault context for an exchange that otherwise hides MCU panic text.
//! Ordinary non-JSON payloads are not echoed into owner diagnostics.
#[derive(Default)]
pub(super) struct RecentFaults {
    lines: std::collections::VecDeque<String>,
    active: bool,
}
impl RecentFaults {
    pub fn remember(&mut self, bytes: &[u8]) {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return;
        };
        let fault = [
            "panicked at",
            "Root UART management failed",
            "memory allocation of",
            "Stack overflow",
            "stack overflow",
            "Backtrace:",
        ]
        .iter()
        .any(|prefix| text.contains(prefix));
        let trace =
            self.active && (text.trim_start().starts_with("0x") || text.contains("esp_backtrace"));
        if fault || trace {
            self.active = true;
            if self.lines.len() == 4 {
                self.lines.remove(1);
            }
            self.lines.push_back(text.chars().take(512).collect());
        }
    }
    pub fn describe(
        &self,
        event: &str,
        error: Box<dyn std::error::Error>,
    ) -> Box<dyn std::error::Error> {
        if self.lines.is_empty() {
            format!("{error} while waiting for {event}").into()
        } else {
            format!(
                "{error} while waiting for {event}; MCU fault: {}",
                self.lines
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" | ")
            )
            .into()
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_recognized_faults_and_bounded_trace_context_survive() {
        let mut faults = RecentFaults::default();
        faults.remember(b"unstructured credential or payload");
        assert!(faults.lines.is_empty());
        faults.remember(b"panicked at source.rs:4: Root UART management failed");
        for _ in 0..8 {
            faults.remember(format!("0x{}", "a".repeat(600)).as_bytes());
        }
        assert_eq!(faults.lines.len(), 4);
        assert!(faults.lines.iter().all(|line| line.len() <= 512));
        assert!(faults.lines[0].contains("Root UART management failed"));
        assert!(faults
            .describe("native-assignment-stage", "UART timeout".into())
            .to_string()
            .contains("native-assignment-stage"));
    }
}
