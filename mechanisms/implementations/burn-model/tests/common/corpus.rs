use crate::common::{request, value};
use conduit_ai::TrainingExampleIdentityPages;
use conduit_burn_model::{ModelBatch, TrainingContext};

use conduit_burn_model::{CorpusBatch, CorpusLimits, CorpusRecipe, PreparedTrainingCorpus};
pub fn finite_corpus_order(
    context: &TrainingContext,
    seed: Option<u64>,
    changed: bool,
) -> PreparedTrainingCorpus {
    let batches = (0..2)
        .map(|i| {
            let mut identity = request(1).batch;
            identity.identity = [31 + i; 32];
            identity.example_identities =
                TrainingExampleIdentityPages::from_values(vec![[10 + 2 * i; 32], [11 + 2 * i; 32]])
                    .unwrap();
            identity.encoded_bytes = 16;
            CorpusBatch {
                identity,
                tensors: ModelBatch {
                    inputs: vec![value(&[i as f32, i as f32 + 1.], 2)],
                    targets: vec![value(
                        &[
                            2. * i as f32 + 1.,
                            2. * i as f32 + 3. + if changed { 1. } else { 0. },
                        ],
                        2,
                    )],
                },
            }
        })
        .collect();
    PreparedTrainingCorpus::prepare(
        context,
        batches,
        CorpusRecipe {
            shuffle_seed: seed,
            epochs: 4,
            allow_short_final_batch: false,
        },
        CorpusLimits {
            maximum_batches: 2,
            maximum_tensor_bytes: 32,
            maximum_epochs: 4,
        },
    )
    .unwrap()
}

pub fn finite_corpus(
    context: &TrainingContext,
    seed: u64,
    changed: bool,
) -> PreparedTrainingCorpus {
    finite_corpus_order(context, Some(seed), changed)
}
