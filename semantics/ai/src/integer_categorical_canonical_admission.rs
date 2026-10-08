/// Fixed canonical numerical admission; the shared model resource is charged
/// separately by its owner. This component executes no target Plan.
#[derive(Clone, Copy, Debug)]
pub struct CategoricalCanonicalAdmissionLimits {
    pub maximum_preparation_peak_bytes: usize,
    pub maximum_retained_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct CategoricalCanonicalAdmissionReceipt {
    pub preparation_peak_heap_bytes_bound: usize,
    pub retained_heap_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CategoricalCanonicalAdmissionRefusal {
    Shape,
    Pressure,
    Preparation,
    Input,
    Inference,
}
pub struct PreparedCategoricalCanonicalAdmission {
    profile: Arc<PreparedCategoricalStep>,
    input: IntegerCollectionCodec,
    output: IntegerCollectionCodec,
    indices: Vec<u64>,
    scores: Vec<i64>,
    receipt: CategoricalCanonicalAdmissionReceipt,
}
impl PreparedCategoricalCanonicalAdmission {
    pub fn prepare(
        profile: Arc<PreparedCategoricalStep>,
        limits: CategoricalCanonicalAdmissionLimits,
    ) -> Result<Self, CategoricalCanonicalAdmissionRefusal> {
        use CategoricalCanonicalAdmissionRefusal as R;
        let (_, outputs, lookups) = profile.dimensions();
        // Four bounded template builds (two codecs, each zero/marker), at most
        // 128 primitive values apiece. This ceiling includes geometric collector
        // and canonical writer requests, all cloned primitive Type owners, both
        // retained templates/encodings, offsets, and inference vectors. It is
        // admitted before the first codec allocates. No model metadata is charged
        // here: the profile was already prepared and remains independently owned.
        const PREPARATION: usize = 4 * 1024 * 1024;
        if outputs == 0
            || outputs > 128
            || lookups == 0
            || lookups > 64
            || core::mem::size_of::<StructuredInfoValue>() > 256
            || core::mem::size_of::<StructuredInfoType>() > 128
            || core::mem::size_of::<usize>() > 16
        {
            return Err(R::Shape);
        }
        if limits.maximum_preparation_peak_bytes < PREPARATION {
            return Err(R::Pressure);
        }
        let check = |ty: &StructuredInfoType, width: usize, kind: &str| {
            matches!(ty.shape(),StructuredInfoTypeShape::Collection{element,length}
                if usize::from(length)==width && matches!(element.shape(),StructuredInfoTypeShape::Leaf(id) if id.as_str()==kind))
        };
        if !check(profile.indices_type(), lookups, "value/u64")
            || !check(profile.scores_type(), outputs, "value/i64")
        {
            return Err(R::Shape);
        }
        // The declared retained ceiling is checked against the same complete
        // preparation bound before allocating, then against actual capacities.
        if limits.maximum_retained_bytes < PREPARATION {
            return Err(R::Pressure);
        }
        let input = IntegerCollectionCodec::prepare(profile.indices_type(), lookups)
            .map_err(|_| R::Preparation)?;
        let output = IntegerCollectionCodec::prepare(profile.scores_type(), outputs)
            .map_err(|_| R::Preparation)?;
        let indices = vec![0; lookups];
        let scores = vec![0; outputs];
        let codec_bytes = |codec: &IntegerCollectionCodec| {
            codec
                .template
                .capacity()
                .checked_add(codec.encoded.capacity())
                .and_then(|n| {
                    codec
                        .offsets
                        .capacity()
                        .checked_mul(core::mem::size_of::<usize>())
                        .and_then(|extra| n.checked_add(extra))
                })
        };
        let retained = codec_bytes(&input)
            .and_then(|n| codec_bytes(&output).and_then(|m| n.checked_add(m)))
            .and_then(|n| {
                indices
                    .capacity()
                    .checked_mul(core::mem::size_of::<u64>())
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| {
                scores
                    .capacity()
                    .checked_mul(core::mem::size_of::<i64>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or(R::Pressure)?;
        if retained > limits.maximum_retained_bytes || retained > PREPARATION {
            return Err(R::Pressure);
        }
        Ok(Self {
            profile,
            input,
            output,
            indices,
            scores,
            receipt: CategoricalCanonicalAdmissionReceipt {
                preparation_peak_heap_bytes_bound: PREPARATION,
                retained_heap_bytes: retained,
            },
        })
    }
    pub fn storage_receipt(&self) -> CategoricalCanonicalAdmissionReceipt {
        self.receipt
    }
    pub fn profile(&self) -> &Arc<PreparedCategoricalStep> {
        &self.profile
    }
    /// Complete original canonical indices are checked against every fixed byte
    /// outside payload offsets before exact adopted-model inference. The whole
    /// returned canonical score frame remains in preallocated owner storage.
    pub fn evaluate<'a>(
        &'a mut self,
        input: &[u8],
    ) -> Result<&'a [u8], CategoricalCanonicalAdmissionRefusal> {
        use CategoricalCanonicalAdmissionRefusal as R;
        self.input
            .decode(input, &mut self.indices)
            .map_err(|_| R::Input)?;
        self.profile
            .infer_indices_into(&self.indices, &mut self.scores)
            .map_err(|_| R::Inference)?;
        Ok(self.output.encode(&self.scores))
    }
    pub fn scores(&self) -> &[i64] {
        &self.scores
    }
}
