//! Exact finite-profile IPA partitioning. This layer assigns notation units,
//! never inventory membership, phonological identity or speaking permission.
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UnitSpelling<'a> {
    pub spelling: &'a str,
    pub unit: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UnitSpan {
    pub unit: usize,
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PartitionRefusal {
    Empty,
    Bound,
    InvalidProfile,
    Unsupported,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PartitionDiagnostic {
    pub refusal: PartitionRefusal,
    pub start: usize,
    pub end: usize,
}

#[cfg(test)]
fn partition(
    source: &str,
    spellings: &[UnitSpelling<'_>],
) -> Result<Vec<UnitSpan>, PartitionRefusal> {
    partition_located(source, spellings).map_err(|error| error.refusal)
}

/// Preserve original byte spans, including combining marks and tie bars.
/// Count complete partitions, capped at two; never resolve ambiguity by greedy
/// longest-match, Unicode normalization, scalar splitting or truncation.
pub(crate) fn partition_located(
    source: &str,
    spellings: &[UnitSpelling<'_>],
) -> Result<Vec<UnitSpan>, PartitionDiagnostic> {
    use PartitionRefusal::*;
    let whole = |refusal| PartitionDiagnostic {
        refusal,
        start: 0,
        end: source.len(),
    };
    if source.is_empty() {
        return Err(whole(Empty));
    }
    if source.len() > 4096 || spellings.len() > 512 {
        return Err(whole(Bound));
    }
    for (index, item) in spellings.iter().enumerate() {
        if item.spelling.is_empty()
            || item.spelling.len() > 64
            || spellings[..index]
                .iter()
                .any(|prior| prior.spelling == item.spelling)
        {
            return Err(whole(InvalidProfile));
        }
    }
    let mut counts = alloc::vec![0_u8; source.len() + 1];
    let mut next = alloc::vec![None; source.len() + 1];
    counts[source.len()] = 1;
    for start in (0..source.len())
        .rev()
        .filter(|&index| source.is_char_boundary(index))
    {
        for item in spellings {
            if !source[start..].starts_with(item.spelling) {
                continue;
            }
            let end = start + item.spelling.len();
            let suffix = counts[end];
            if suffix == 0 {
                continue;
            }
            if counts[start] == 0 && suffix == 1 {
                next[start] = Some(UnitSpan {
                    unit: item.unit,
                    start,
                    end,
                });
            }
            counts[start] = counts[start].saturating_add(suffix).min(2);
        }
    }
    match counts[0] {
        0 => {
            // Find the furthest exact declared prefix; underline the first
            // scalar which cannot continue it, including an orphan combining mark.
            let mut reachable = alloc::vec![false; source.len() + 1];
            reachable[0] = true;
            let mut furthest = 0;
            for start in 0..source.len() {
                if !reachable[start] {
                    continue;
                }
                for item in spellings {
                    if source[start..].starts_with(item.spelling) {
                        let end = start + item.spelling.len();
                        reachable[end] = true;
                        furthest = furthest.max(end);
                    }
                }
            }
            let end = furthest + source[furthest..].chars().next().map_or(0, char::len_utf8);
            return Err(PartitionDiagnostic {
                refusal: Unsupported,
                start: furthest,
                end,
            });
        }
        2 => {
            // Skip the common unambiguous prefix; the remaining suffix has
            // multiple complete partitions. Never choose a longest match.
            let mut start = 0;
            loop {
                let mut choices = spellings.iter().filter(|item| {
                    source[start..].starts_with(item.spelling)
                        && counts[start + item.spelling.len()] != 0
                });
                let first = choices.next().expect("a complete partition exists");
                if choices.next().is_some() {
                    break;
                }
                start += first.spelling.len();
            }
            return Err(PartitionDiagnostic {
                refusal: Ambiguous,
                start,
                end: source.len(),
            });
        }
        _ => {}
    }
    let mut spans = Vec::new();
    let mut start = 0;
    while start < source.len() {
        if spans.len() == 256 {
            let end = next[start].map_or(start, |span| span.end);
            return Err(PartitionDiagnostic {
                refusal: Bound,
                start,
                end,
            });
        }
        let span = next[start].ok_or_else(|| whole(Unsupported))?;
        start = span.end;
        spans.push(span);
    }
    Ok(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multicodepoint_units_and_explicit_alias_preserve_source() {
        let units = ["t͡ʃ", "d͡ʒ", "pʰ", "n̩", "ã", "ã", "ˈ", "ˌ", "ː"];
        let profile: Vec<_> = units
            .iter()
            .enumerate()
            .map(|(unit, spelling)| UnitSpelling {
                spelling,
                unit: if unit == 5 { 4 } else { unit },
            })
            .collect();
        let source = "ˈt͡ʃpʰn̩ããːˌd͡ʒ";
        let spans = partition(source, &profile).unwrap();
        let recovered: alloc::string::String = spans
            .iter()
            .map(|span| &source[span.start..span.end])
            .collect();
        assert_eq!(recovered, source);
        assert_eq!(spans[4].unit, spans[5].unit);
        assert_ne!(
            &source[spans[4].start..spans[4].end],
            &source[spans[5].start..spans[5].end]
        );
    }
    #[test]
    fn ambiguous_complete_partition_refuses_instead_of_longest_match() {
        let profile = [
            UnitSpelling {
                spelling: "ts",
                unit: 0,
            },
            UnitSpelling {
                spelling: "t",
                unit: 1,
            },
            UnitSpelling {
                spelling: "s",
                unit: 2,
            },
        ];
        assert_eq!(partition("ts", &profile), Err(PartitionRefusal::Ambiguous));
        assert_eq!(partition("θ", &profile), Err(PartitionRefusal::Unsupported));
    }
    #[test]
    fn no_partial_success_or_silent_truncation() {
        let profile = [UnitSpelling {
            spelling: "ɪ",
            unit: 0,
        }];
        assert_eq!(
            partition("ɪx", &profile),
            Err(PartitionRefusal::Unsupported)
        );
        assert_eq!(
            partition(&"ɪ".repeat(257), &profile),
            Err(PartitionRefusal::Bound)
        );
        assert_eq!(
            partition("ɪ", &[profile[0], profile[0]]),
            Err(PartitionRefusal::InvalidProfile)
        );
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    fn unsupported_combining_mark_has_an_exact_utf8_span() {
        let profile = [UnitSpelling {
            spelling: "ã",
            unit: 0,
        }];
        let error = partition_located("ã̃", &profile).unwrap_err();
        assert_eq!(
            error,
            PartitionDiagnostic {
                refusal: PartitionRefusal::Unsupported,
                start: 2,
                end: 4
            }
        );
    }
    #[test]
    fn ambiguity_skips_a_common_declared_prefix() {
        let profile = [
            UnitSpelling {
                spelling: "x",
                unit: 0,
            },
            UnitSpelling {
                spelling: "ts",
                unit: 1,
            },
            UnitSpelling {
                spelling: "t",
                unit: 2,
            },
            UnitSpelling {
                spelling: "s",
                unit: 3,
            },
        ];
        assert_eq!(
            partition_located("xts", &profile).unwrap_err(),
            PartitionDiagnostic {
                refusal: PartitionRefusal::Ambiguous,
                start: 1,
                end: 3
            }
        );
    }
}
