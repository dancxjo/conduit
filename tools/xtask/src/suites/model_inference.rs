//! Imported model admission and real named-I/O execution; no ORT or remote fetch claim.
use crate::process::Step;
pub const STEPS: &[Step] = &[Step::new(
    "model.named-inference",
    "Admit immutable model signatures and execute the prepared named-tensor profile through Source, Plan and the production kernel",
    "cargo",
    &[
        "test", "--locked",
        "-p", "conduit-core", "--test", "exact_realization_property",
        "-p", "conduit-planner", "--test", "model_signature_realization",
        "-p", "conduit-data", "--test", "named_tensor_view", "--test", "tensor_view_axis", "--test", "tensor", "--test", "tensor_scalar",
        "-p", "conduit-ai", "--test", "native_model_signature_vocabulary",
        "--test", "onnx_metadata", "--test", "imported_back_admission", "--test", "imported_onnx", "--test", "onnx_tensor_io",
        "--test", "model_infer_fore", "--test", "model_tensor_io", "--test", "prepared_model_tensor_io",
        "-p", "conduit-std-host", "--test", "model_compute_host", "--test", "model_infer_plot",
        "--test", "model_infer_onnx", "--test", "model_infer_onnx_scalar", "--test", "model_infer_onnx_dynamic", "--test", "model_infer_imported_refusals", "--test", "model_infer_terminals", "--test", "model_infer_output_template",
        "--test", "model_infer_allocation", "--test", "model_infer_variable_inputs",
        "--", "--nocapture",
    ],
)];
