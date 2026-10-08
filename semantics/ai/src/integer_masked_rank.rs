//! Finite integer ranking. Domain owners supply the admissibility mask.
use alloc::vec::Vec;

pub const INTEGER_MASKED_RANK_PROFILE: &str = "ai/integer-masked-rank@1";
pub const MAXIMUM_INTEGER_RANK_ITEMS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerMaskedRankRefusal {
    Shape,
    Limit,
}

/// Descending score, then ascending input index; masked items never survive.
pub fn integer_masked_top_k(
    scores: &[i64],
    allowed: &[bool],
    maximum: usize,
) -> Result<Vec<usize>, IntegerMaskedRankRefusal> {
    if scores.len() != allowed.len() || scores.len() > MAXIMUM_INTEGER_RANK_ITEMS {
        return Err(IntegerMaskedRankRefusal::Shape);
    }
    if maximum > MAXIMUM_INTEGER_RANK_ITEMS {
        return Err(IntegerMaskedRankRefusal::Limit);
    }
    let mut indices = allowed
        .iter()
        .enumerate()
        .filter_map(|(index, allowed)| allowed.then_some(index))
        .collect::<Vec<_>>();
    indices.sort_by(|left, right| scores[*right].cmp(&scores[*left]).then(left.cmp(right)));
    indices.truncate(maximum);
    Ok(indices)
}

/// Rank into caller-prepared finite storage without allocating during inference.
/// The returned prefix has the same order as `integer_masked_top_k` with
/// `maximum = output.len()`; unused output slots carry no result.
pub fn integer_masked_top_k_into(
    scores: &[i64],
    allowed: &[bool],
    output: &mut [usize],
) -> Result<usize, IntegerMaskedRankRefusal> {
    if scores.len() != allowed.len() || scores.len() > MAXIMUM_INTEGER_RANK_ITEMS {
        return Err(IntegerMaskedRankRefusal::Shape);
    }
    if output.len() > MAXIMUM_INTEGER_RANK_ITEMS {
        return Err(IntegerMaskedRankRefusal::Limit);
    }
    let mut length = 0;
    for (index, admitted) in allowed.iter().copied().enumerate() {
        if !admitted {
            continue;
        }
        let position = output[..length]
            .iter()
            .position(|previous| {
                scores[index] > scores[*previous]
                    || (scores[index] == scores[*previous] && index < *previous)
            })
            .unwrap_or(length);
        if position >= output.len() {
            continue;
        }
        let next_length = (length + 1).min(output.len());
        output.copy_within(position..next_length - 1, position + 1);
        output[position] = index;
        length = next_length;
    }
    Ok(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_ties_and_extreme_scores_have_deterministic_finite_order() {
        assert_eq!(
            integer_masked_top_k(&[i64::MAX, 7, 7, i64::MIN], &[false, true, true, true], 4),
            Ok(alloc::vec![1, 2, 3])
        );
        assert_eq!(integer_masked_top_k(&[7], &[false], 4), Ok(Vec::new()));
        assert_eq!(integer_masked_top_k(&[7], &[true], 0), Ok(Vec::new()));
        assert_eq!(
            integer_masked_top_k(&[7], &[], 4),
            Err(IntegerMaskedRankRefusal::Shape)
        );
        assert_eq!(
            integer_masked_top_k(&[], &[], 129),
            Err(IntegerMaskedRankRefusal::Limit)
        );
    }
    #[test]
    fn prepared_storage_matches_ranking_for_masks_ties_and_all_finite_limits() {
        let scores = [i64::MIN, 8, 8, i64::MAX, -4, 8];
        for mask in 0u64..64 {
            let allowed = core::array::from_fn::<_, 6, _>(|i| mask & (1 << i) != 0);
            for maximum in 0..=MAXIMUM_INTEGER_RANK_ITEMS {
                let expected = integer_masked_top_k(&scores, &allowed, maximum).unwrap();
                let mut storage = [usize::MAX; MAXIMUM_INTEGER_RANK_ITEMS];
                let length =
                    integer_masked_top_k_into(&scores, &allowed, &mut storage[..maximum]).unwrap();
                assert_eq!(&storage[..length], expected);
            }
        }
        let mut oversized = [0; MAXIMUM_INTEGER_RANK_ITEMS + 1];
        assert_eq!(
            integer_masked_top_k_into(&[], &[], &mut oversized),
            Err(IntegerMaskedRankRefusal::Limit)
        );
        assert_eq!(
            integer_masked_top_k_into(&[0], &[], &mut []),
            Err(IntegerMaskedRankRefusal::Shape)
        );
    }
}
