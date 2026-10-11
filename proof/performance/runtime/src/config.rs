//! The same finite command contract is used by both xtask dispatchers and worker.
use std::path::PathBuf;
#[derive(Debug, Clone, serde::Serialize)]
pub struct Config {
    pub output: PathBuf,
    pub values: u32,
    pub repetitions: u16,
    pub decisions: u32,
    pub samples: u32,
}
impl Config {
    pub fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut config = Self {
            output: PathBuf::from("target/runtime-performance"),
            values: 1_000_000,
            repetitions: 5,
            decisions: 10_000,
            samples: 2_000,
        };
        let mut seen = std::collections::BTreeSet::new();
        let mut arguments = arguments.iter();
        while let Some(name) = arguments.next() {
            if !seen.insert(name) {
                return Err(format!("duplicate option {name}"));
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {name}"))?;
            match name.as_str() {
                "--output" => config.output = value.into(),
                "--values" => config.values = value.parse().map_err(|_| "invalid --values")?,
                "--repetitions" => {
                    config.repetitions = value.parse().map_err(|_| "invalid --repetitions")?
                }
                "--decisions" => {
                    config.decisions = value.parse().map_err(|_| "invalid --decisions")?
                }
                "--samples" => config.samples = value.parse().map_err(|_| "invalid --samples")?,
                _ => return Err(format!("unknown runtime-performance option {name}")),
            }
        }
        if !(1_000_000..=10_000_000).contains(&config.values)
            || !(3..=15).contains(&config.repetitions)
            || !(1_000..=1_000_000).contains(&config.decisions)
            || !(1_000..=100_000).contains(&config.samples)
        {
            return Err("finite profile: values 1000000..10000000, repetitions 3..15, decisions 1000..1000000, samples 1000..100000".into());
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn finite_admission_refuses_unbounded_or_ambiguous_runs() {
        let parse = |values: &[&str]| {
            Config::parse(&values.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>())
        };
        assert!(parse(&[]).is_ok());
        for options in [
            vec!["--values", "999999"],
            vec!["--decisions", "1000001"],
            vec!["--repetitions", "2"],
            vec!["--samples", "0"],
            vec!["--values", "1000000", "--values", "1000000"],
            vec!["--output"],
            vec!["--unknown", "1"],
        ] {
            assert!(parse(&options).is_err(), "{options:?}");
        }
    }
}
