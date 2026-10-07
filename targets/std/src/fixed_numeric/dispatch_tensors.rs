use super::*;
use conduit_ai::fixed_numeric_preparation::fixed_affine_offer;
use conduit_ai::{
    fixed_numeric_back::*, fixed_numeric_linear_back::*, fixed_numeric_operations_back::*,
};
pub(super) fn select(
    kind: &str,
    planned: Option<(&PlannedGear, u16)>,
    bindings: &TensorBindings,
) -> Option<Result<Selection, String>> {
    match kind {
        "numeric/dense161x18" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<161, 18>(),
            FixedAffineBack::<161, 18>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/linear161x18" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<161, 18>(),
                FixedLinearBack::<161, 18>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/dense18x18" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<18, 18>(),
            FixedAffineBack::<18, 18>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/linear18x18" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<18, 18>(),
                FixedLinearBack::<18, 18>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/dense3x2" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<3, 2>(),
            FixedAffineBack::<3, 2>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense32x64" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<32, 64>(),
            FixedAffineBack::<32, 64>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense192x128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<192, 128>(),
            FixedAffineBack::<192, 128>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense128x320" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<128, 320>(),
            FixedAffineBack::<128, 320>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense80x1" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<80, 1>(),
            FixedAffineBack::<80, 1>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense328x192" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<328, 192>(),
            FixedAffineBack::<328, 192>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense192x192" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<192, 192>(),
            FixedAffineBack::<192, 192>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense192x4" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<192, 4>(),
            FixedAffineBack::<192, 4>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense160x160" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<160, 160>(),
            FixedAffineBack::<160, 160>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense128x128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<128, 128>(),
            FixedAffineBack::<128, 128>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense688x128" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<688, 128>(),
            FixedAffineBack::<688, 128>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/dense128x40" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_affine_offer::<128, 40>(),
            FixedAffineBack::<128, 40>::prepare_planned_owned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
                gear,
                fuel,
                resource(bindings, "weights")?,
                resource(bindings, "bias")?
            )
        )),
        "numeric/linear80x1" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<80, 1>(),
                FixedLinearBack::<80, 1>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear328x192" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<328, 192>(),
                FixedLinearBack::<328, 192>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear192x192" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<192, 192>(),
                FixedLinearBack::<192, 192>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear192x4" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<192, 4>(),
                FixedLinearBack::<192, 4>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear272x480" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<272, 480>(),
                FixedLinearBack::<272, 480>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear160x480" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<160, 480>(),
                FixedLinearBack::<160, 480>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear240x384" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<240, 384>(),
                FixedLinearBack::<240, 384>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear128x384" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<128, 384>(),
                FixedLinearBack::<128, 384>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear208x384" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<208, 384>(),
                FixedLinearBack::<208, 384>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear160x160" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<160, 160>(),
                FixedLinearBack::<160, 160>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear128x128" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<128, 128>(),
                FixedLinearBack::<128, 128>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear688x128" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<688, 128>(),
                FixedLinearBack::<688, 128>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/linear128x40" => {
            Some(run!(
                planned,
                gear,
                fuel,
                fixed_linear_offer::<128, 40>(),
                FixedLinearBack::<128, 40>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
            ))
        }
        "numeric/embedding224x12" => Some(run!(
            planned,
            gear,
            fuel,
            fixed_embedding_offer::<224, 12>(),
            FixedEmbeddingBack::<224, 12>::prepare_planned_owned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel, resource(bindings, "weights")?)
        )),
        _ => None,
    }
}
