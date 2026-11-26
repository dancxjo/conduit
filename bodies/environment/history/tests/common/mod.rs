use conduit_dht11_observation::{ObservationContract, PreparedObservationQuantityForms};
use conduit_environment_history::*;
use conduit_environment_runtime::recording::{PreparedRecordingChunk, RecordingDisposition as D};
mod identity;
pub use identity::{basis, context};
pub fn chunk(events: &[(u64, u64, bool)]) -> String {
    let contract = ObservationContract::prepare().unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap().to_vec();
    let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    for &(seq, time, available) in events {
        chunk
            .append(
                seq,
                time,
                if available { D::Available } else { D::Checksum },
                available.then_some(observation.as_slice()),
            )
            .unwrap();
    }
    chunk.seal().unwrap().into()
}
pub fn prepared(chunks: u16) -> PreparedHistoryFace {
    PreparedHistoryFace::prepare(context(), &ObservationContract::prepare().unwrap(), chunks)
        .unwrap()
}
