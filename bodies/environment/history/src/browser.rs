//! A bounded HTML/SVG preparation adapter, without browser execution or persistence.
use crate::graph::HistoryGraph;
use alloc::{format, string::String};
use core::fmt::Write;
impl HistoryGraph<'_> {
    pub fn browser_html(&self) -> String {
        let mut out = format!("<section><h2>Room conditions history</h2><p>Recovered readings. {} samples outside this view. Dots are observations; missing readings are never interpolated.</p><details><summary>Inspect source</summary><p>Face {} revision {}. Producer Host {}; Boot {}; Plan {}; Play {}. Retrieved recording content is validated; this view does not attest a durable commit.</p></details><svg viewBox=\"0 0 640 340\" role=\"img\" aria-label=\"Temperature and humidity over producer Boot time\"><text x=\"32\" y=\"20\">Temperature (°C)</text><text x=\"32\" y=\"170\">Relative humidity (%)</text>", self.omitted, escape(self.content_id), self.revision, escape(self.producer_context[0]), escape(self.producer_context[1]), escape(self.producer_context[2]), escape(self.producer_context[3]));
        if self.samples.is_empty() {
            out.push_str("</svg><p>No recovered samples in this view.</p></section>");
            return out;
        }
        let (min, max) = self.temperature_range();
        let first = self.samples.first().map_or(0, |s| s.boot_ms);
        let last = self.samples.last().map_or(0, |s| s.boot_ms);
        let _ = write!(out, "<text x=\"32\" y=\"35\">top {}; bottom {}</text><text x=\"32\" y=\"185\">top 100; bottom 0</text><text x=\"32\" y=\"325\">Boot ms: left {first}; right {last}</text>", super::graph::milli_label(max), super::graph::milli_label(min));
        for sample in &self.samples {
            let x = self.x(sample);
            for (value, humidity) in [
                (sample.temperature_milli_c, false),
                (sample.humidity_milli_percent, true),
            ] {
                if let Some(value) = value {
                    let _ = write!(out, "<circle cx=\"{x}\" cy=\"{}\" r=\"3\"><title>sequence {}; {} ms; {} {}</title></circle>", self.y(value, humidity), sample.sequence, sample.boot_ms, super::graph::milli_label(value), if humidity { "%" } else { "C" });
                }
            }
        }
        out.push_str("</svg><ul>");
        for sample in &self.samples {
            if sample.failure || sample.gap_before.is_some() {
                let _ = write!(
                    out,
                    "<li>Sequence {}, {} ms: {}",
                    sample.sequence,
                    sample.boot_ms,
                    if sample.failure {
                        "sensor unavailable"
                    } else {
                        "available"
                    }
                );
                if let Some((first, last)) = sample.gap_before {
                    let _ = write!(out, "; missing sequences {first} through {last}");
                }
                out.push_str("</li>");
            }
        }
        out.push_str("</ul><table><caption>Shown recovered samples</caption><thead><tr><th>Sequence</th><th>Producer Boot ms</th><th>Temperature °C</th><th>Humidity %</th><th>Sensor status</th></tr></thead><tbody>");
        for sample in &self.samples {
            let temperature = sample.temperature_milli_c.map(super::graph::milli_label);
            let humidity = sample.humidity_milli_percent.map(super::graph::milli_label);
            let _ = write!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                sample.sequence,
                sample.boot_ms,
                temperature.as_deref().unwrap_or("unavailable"),
                humidity.as_deref().unwrap_or("unavailable"),
                sample.failure_reason.unwrap_or("available")
            );
        }
        out.push_str("</tbody></table></section>");
        out
    }
}
fn escape(value: &str) -> String {
    let mut result = String::new();
    for c in value.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#39;"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn labels_escape_html_syntax() {
        assert_eq!(
            super::escape("<script a='x'>&\""),
            "&lt;script a=&#39;x&#39;&gt;&amp;&quot;"
        );
    }
}
