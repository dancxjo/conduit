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
}
