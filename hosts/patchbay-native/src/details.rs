//! Bounded lenses over the existing deterministic linear Patchbay truth.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum DetailsLens {
    #[default]
    Source,
    Checked,
    Plan,
    Play,
    Signs,
    Trace,
    All,
}

impl DetailsLens {
    pub(super) const ALL: [Self; 7] = [
        Self::Source,
        Self::Checked,
        Self::Plan,
        Self::Play,
        Self::Signs,
        Self::Trace,
        Self::All,
    ];

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Source => "SOURCE / READ ONLY",
            Self::Checked => "CHECKED",
            Self::Plan => "PLAN",
            Self::Play => "PLAY",
            Self::Signs => "SIGNS",
            Self::Trace => "TRACE",
            Self::All => "ALL / LINEAR TRUTH",
        }
    }

    pub(super) fn move_by(&mut self, delta: isize) {
        let current = Self::ALL.iter().position(|lens| lens == self).unwrap_or(0) as isize;
        let last = Self::ALL.len() as isize - 1;
        *self = Self::ALL[(current + delta).clamp(0, last) as usize];
    }
}

pub(super) fn lens_lines(lens: DetailsLens, complete: &[String]) -> Vec<String> {
    let mut lines = vec![format!("DETAILS / {}", lens.label())];
    lines.push("LEFT/RIGHT LENS  F2 CLOSE  SOURCE CANNOT BE TYPED INTO".into());
    if lens == DetailsLens::All {
        lines.extend_from_slice(complete);
        return lines;
    }
    lines.extend(complete.iter().filter(|line| belongs(lens, line)).cloned());
    if lines.len() == 2 {
        lines.push("  no facts in this lens".into());
    }
    lines
}

fn belongs(lens: DetailsLens, line: &str) -> bool {
    match lens {
        DetailsLens::Source => {
            line.starts_with("SOURCE ")
                || line.starts_with("  form ")
                || line.starts_with("    ")
                || line == "  }"
        }
        DetailsLens::Checked => {
            line.starts_with("CHECKED")
                || line.starts_with("DIAGNOSTIC")
                || line.starts_with("> ")
                || line.starts_with("  face-")
                || line.starts_with("  startup")
                || line.starts_with("  gear")
                || line.starts_with("  cord")
        }
        DetailsLens::Plan => line.starts_with("PLAN") || line.contains(" plan="),
        DetailsLens::Play => line.starts_with("PLAY") || line.contains(" play="),
        DetailsLens::Signs => line.starts_with("SIGN") || line.contains(" sign="),
        DetailsLens::Trace => {
            line.starts_with("INTERACTION")
                || line.starts_with("REQUEST")
                || line.starts_with("RECEIPT")
                || line.starts_with("ROUTE")
                || line.starts_with("LINE")
                || line.starts_with("RENDERER")
                || line.starts_with("MANIFESTATION")
        }
        DetailsLens::All => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_is_finite_nonwrapping_and_all_retains_every_line() {
        let mut lens = DetailsLens::Source;
        lens.move_by(-1);
        assert_eq!(lens, DetailsLens::Source);
        for _ in 0..20 {
            lens.move_by(1);
        }
        assert_eq!(lens, DetailsLens::All);
        let complete = vec![
            "SOURCE x revision=1".into(),
            "PLAN p".into(),
            "SIGN s".into(),
        ];
        let projected = lens_lines(lens, &complete);
        assert_eq!(&projected[2..], complete);
    }
}
