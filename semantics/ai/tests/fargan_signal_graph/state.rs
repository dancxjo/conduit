use conduit_core::*;
#[derive(Clone, Debug)]
pub struct State {
    pub conv: [f32; 164],
    pub gru1: [f32; 160],
    pub gru2: [f32; 128],
    pub gru3: [f32; 128],
    pub pitch: [f32; 256],
    pub deemphasis: f32,
}
pub fn float_tree(
    ty: &StructuredInfoType,
    values: &mut impl Iterator<Item = f32>,
) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), float_tree(representation, values)).unwrap()
        }
        StructuredInfoTypeShape::Collection { element, length } => StructuredInfoValue::collection(
            ty.clone(),
            (0..length).map(|_| float_tree(element, values)).collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == F32_INFO_ID => {
            let value = values.next().unwrap();
            assert!(value.is_finite());
            StructuredInfoValue::leaf(ty.clone(), value.to_le_bytes().to_vec()).unwrap()
        }
        _ => panic!("exact finite vector fixture"),
    }
}
pub fn vector(ty: &StructuredInfoType, values: &[f32]) -> StructuredInfoValue {
    let mut values = values.iter().copied();
    let result = float_tree(ty, &mut values);
    assert!(values.next().is_none());
    result
}
fn record(
    ty: &StructuredInfoType,
    mut value: impl FnMut(&str, &StructuredInfoType) -> StructuredInfoValue,
) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), record(representation, value)).unwrap()
        }
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(field.name(), value(field.name(), field.value_type()))
                        .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        _ => panic!("exact state record fixture"),
    }
}
pub fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
pub fn floats(value: &StructuredInfoValue) -> Vec<f32> {
    match value.shape() {
        StructuredInfoValueShape::Collection(values) => values.iter().flat_map(floats).collect(),
        StructuredInfoValueShape::Leaf(bytes) => {
            let value = f32::from_le_bytes(bytes.try_into().unwrap());
            assert!(value.is_finite());
            vec![value]
        }
        _ => panic!("finite f32 vector"),
    }
}
impl State {
    pub fn encode(&self, ty: &StructuredInfoType) -> Vec<u8> {
        record(ty, |name, ty| match name {
            "deemphasis_state" => vector(ty, &[self.deemphasis]),
            "pitch_history" => vector(ty, &self.pitch),
            "signal" => record(ty, |name, ty| {
                vector(
                    ty,
                    match name {
                        "conv_history" => &self.conv[..],
                        "gru1" => &self.gru1[..],
                        "gru2" => &self.gru2[..],
                        "gru3" => &self.gru3[..],
                        _ => panic!("state field"),
                    },
                )
            }),
            _ => panic!("state field"),
        })
        .canonical_bytes()
        .unwrap()
    }
    pub fn from_result(value: &StructuredInfoValue) -> Self {
        Self::from_state(field(value, "next_state"))
    }
    pub fn from_state(next: &StructuredInfoValue) -> Self {
        let signal = field(next, "signal");
        Self {
            conv: floats(field(signal, "conv_history")).try_into().unwrap(),
            gru1: floats(field(signal, "gru1")).try_into().unwrap(),
            gru2: floats(field(signal, "gru2")).try_into().unwrap(),
            gru3: floats(field(signal, "gru3")).try_into().unwrap(),
            pitch: floats(field(next, "pitch_history")).try_into().unwrap(),
            deemphasis: floats(field(next, "deemphasis_state"))[0],
        }
    }
    pub fn from_oracle(values: &[f32]) -> Self {
        assert_eq!(values.len(), 837);
        Self {
            conv: values[..164].try_into().unwrap(),
            gru1: values[164..324].try_into().unwrap(),
            gru2: values[324..452].try_into().unwrap(),
            gru3: values[452..580].try_into().unwrap(),
            pitch: values[580..836].try_into().unwrap(),
            deemphasis: values[836],
        }
    }
    pub fn flattened(&self) -> Vec<f32> {
        self.conv
            .iter()
            .chain(&self.gru1)
            .chain(&self.gru2)
            .chain(&self.gru3)
            .chain(&self.pitch)
            .chain(core::iter::once(&self.deemphasis))
            .copied()
            .collect()
    }
}
