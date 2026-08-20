use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrowserPointerReceipt {
    pub plan_id: String,
    pub play_id: String,
    pub sign_id: String,
    pub source_placement_id: String,
    pub presentation_placement_id: String,
    pub schema: String,
    pub value_kind: String,
    pub canonical_bytes: usize,
    pub position_x: i64,
    pub position_y: i64,
    pub delta_x: i64,
    pub delta_y: i64,
    pub primary_pressed: bool,
    pub coalesced: u64,
    pub dropped: u64,
    pub queue_capacity: u64,
    pub sequence: u64,
}
