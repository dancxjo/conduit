#[path="../../semantics/language/tests/common/parser_fixture.rs"] mod fixture;
#[path="../../semantics/language/tests/common/parser_model_resource.rs"] mod model_resource;
#[path="../../semantics/language/tests/common/window8_proposal_candidate.rs"] mod candidate;
fn main() { let value=candidate::prepare(); println!("final independently pinned declaration selection ready: {:?}", value.scorer.storage_receipt()); }
