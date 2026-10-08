use super::fixtures::*;
use conduit_core::*;

pub(super) fn synthetic_resources(plan: &Plan) -> Resources {
    let types = conduit_ai::fixed_numeric_catalog::fixed_numeric_types().unwrap();
    plan.fragments[0]
        .placements
        .iter()
        .filter_map(|gear| {
            let name = gear.kind_id.as_str().strip_prefix("epoch-proof/")?;
            if name == "value" || name == "result" {
                return None;
            }
            let native = types.iter().find(|ty| {
                ty.value_type.profile().unwrap().value_kind() == &gear.outputs[0].value_kind
            })?;
            let dimensions: Vec<u64> =
                if let Some(shape) = native.name.strip_prefix("NumericF32MatrixRef") {
                    shape
                        .split('x')
                        .map(|dimension| dimension.parse().unwrap())
                        .collect()
                } else {
                    vec![native
                        .name
                        .strip_prefix("NumericF32BiasRef")
                        .unwrap()
                        .parse()
                        .unwrap()]
                };
            let mut values = vec![0.; dimensions.iter().product::<u64>() as usize];
            if name == "output_bias" {
                values.fill(0.25);
            }
            Some((
                name.into(),
                Resource::new(native.value_type.clone(), &dimensions, values),
            ))
        })
        .collect()
}
