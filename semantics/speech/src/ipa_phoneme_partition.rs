//! Complete phoneme partitioning over already parsed, explicitly bound IPA units.
use crate::{ipa_inventory::IpaInventoryRefusal, semantic::*};
use alloc::{vec, vec::Vec};

#[derive(Clone, Copy)]
pub(super) enum Part {
    Phoneme { binding: usize, length: usize },
    Mark,
}
pub(super) struct PartitionError {
    pub refusal: IpaInventoryRefusal,
    pub start: usize,
    pub end: usize,
}

pub(super) fn partition(
    units: &[SpeechIpaUnitOccurrence],
    profile: &SpeechIpaNotationProfile,
    bindings: &[&SpeechIpaPhonemeDefinitionBinding],
) -> Result<Vec<(usize, Part)>, PartitionError> {
    let kinds: Vec<_> = units
        .iter()
        .map(|unit| {
            profile
                .units()
                .as_slice()
                .iter()
                .find(|definition| definition.identity() == unit.unit())
                .expect("executed notation refers to supplied profile")
                .kind()
        })
        .collect();
    let edges = |offset: usize| -> Vec<(usize, Part)> {
        if matches!(
            kinds[offset],
            SpeechIpaUnitKind::PrimaryStress
                | SpeechIpaUnitKind::SecondaryStress
                | SpeechIpaUnitKind::SyllableBoundary
        ) {
            return vec![(offset + 1, Part::Mark)];
        }
        bindings
            .iter()
            .enumerate()
            .filter_map(|(index, binding)| {
                let expected = binding.units().as_slice();
                let end = offset + expected.len();
                (end <= units.len()
                    && units[offset..end]
                        .iter()
                        .zip(expected)
                        .all(|(actual, expected)| actual.unit() == expected))
                .then_some((
                    end,
                    Part::Phoneme {
                        binding: index,
                        length: expected.len(),
                    },
                ))
            })
            .collect()
    };
    let mut ways = vec![0_u8; units.len() + 1];
    ways[units.len()] = 1;
    for offset in (0..units.len()).rev() {
        for (end, _) in edges(offset) {
            ways[offset] = ways[offset].saturating_add(ways[end]).min(2);
        }
    }
    if ways[0] == 0 {
        let mut reachable = vec![false; units.len() + 1];
        reachable[0] = true;
        let mut furthest = 0;
        for offset in 0..units.len() {
            if reachable[offset] {
                furthest = offset;
                for (end, _) in edges(offset) {
                    reachable[end] = true;
                }
            }
        }
        return Err(PartitionError {
            refusal: IpaInventoryRefusal::UnknownPhoneme,
            start: furthest,
            end: furthest + 1,
        });
    }
    let mut parts = Vec::new();
    let mut offset = 0;
    while offset < units.len() {
        let candidates: Vec<_> = edges(offset)
            .into_iter()
            .filter(|(end, _)| ways[*end] != 0)
            .collect();
        if candidates.len() != 1 {
            return Err(PartitionError {
                refusal: IpaInventoryRefusal::AmbiguousPhoneme,
                start: offset,
                end: units.len(),
            });
        }
        let (end, part) = candidates[0];
        parts.push((offset, part));
        offset = end;
    }
    Ok(parts)
}
