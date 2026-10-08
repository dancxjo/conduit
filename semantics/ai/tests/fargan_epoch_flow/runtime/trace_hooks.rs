//! Prepared bounded diagnostic observers; never recurrent acknowledgements.
use super::*;
// One authoritative development observer implementation is shared by component
// transaction tests and the ordinary scheduler hookup.
pub(super) use super::super::trace_observer::{recorder, sink};

pub(super) struct TraceSinks {
    sinks: BTreeMap<String, Rc<std::cell::RefCell<sink::DevelopmentTraceSink>>>,
}
impl TraceSinks {
    pub(super) fn prepare(
        types: &[(String, StructuredInfoType)],
        anchor: &StructuredInfoValue,
        first_epoch: u64,
        rows: usize,
    ) -> Result<Self, recorder::TraceRefusal> {
        if types.len() != 3 {
            return Err(recorder::TraceRefusal::Preparation);
        }
        let mut sinks = BTreeMap::new();
        for (name, ty) in types {
            if !matches!(
                name.as_str(),
                "feature_trace" | "history_trace" | "pcm_trace"
            ) {
                return Err(recorder::TraceRefusal::Preparation);
            }
            let maximum = conduit_plot::maximum_prepared_transport_value_bytes(ty)
                .map_err(|_| recorder::TraceRefusal::Preparation)?
                as usize;
            let recorder = recorder::DevelopmentTraceRecorder::prepare(
                ty,
                anchor,
                first_epoch,
                rows,
                maximum,
            )?;
            let sink = sink::DevelopmentTraceSink::prepare(recorder, maximum)?;
            if sinks
                .insert(name.clone(), Rc::new(std::cell::RefCell::new(sink)))
                .is_some()
            {
                return Err(recorder::TraceRefusal::Preparation);
            }
        }
        Ok(Self { sinks })
    }
    pub(super) fn get(
        &self,
        name: &str,
    ) -> Option<Rc<std::cell::RefCell<sink::DevelopmentTraceSink>>> {
        self.sinks.get(name).cloned()
    }
    pub(super) fn finished(&self) -> bool {
        self.sinks
            .values()
            .all(|sink| sink.borrow().recorder().finish().is_ok())
    }
    pub(super) fn rows(&self, name: &str) -> Vec<Vec<u8>> {
        self.sinks[name]
            .borrow()
            .recorder()
            .finish()
            .expect("all trace rows committed")
            .to_vec()
    }
}
