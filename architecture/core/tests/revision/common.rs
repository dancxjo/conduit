use conduit_core::revision::*;

pub fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
pub fn context() -> RevisionContext<'static> {
    RevisionContext {
        stream: text("tracking/session/1"),
        subject: text("track/7"),
        epoch: text("history/1"),
        producer: text("sensor/fusion@1"),
        policy: text("tracking/observed-window@1"),
    }
}
pub const EVIDENCE: [RevisionEvidence<'static>; 1] = [RevisionEvidence {
    source: RevisionTextConst::SOURCE,
    generation: RevisionTextConst::GENERATION,
}];
struct RevisionTextConst;
impl RevisionTextConst {
    const SOURCE: RevisionText<'static> = match RevisionText::new("camera/frame") {
        Ok(value) => value,
        Err(_) => panic!(),
    };
    const GENERATION: RevisionText<'static> = match RevisionText::new("frame/content/17") {
        Ok(value) => value,
        Err(_) => panic!(),
    };
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackDelta {
    pub first: Frame,
    pub through: Frame,
    pub class: Option<i16>,
}
pub struct Tracking;
impl RevisionDomain for Tracking {
    type Delta = TrackDelta;
    type Cursor = Frame;
    fn contract(&self) -> RevisionText<'_> {
        text("tracking/classification-revision@1")
    }
    fn validate_delta(&self, role: RevisionDeltaRole, delta: &TrackDelta) -> bool {
        if role == RevisionDeltaRole::Withdrawal {
            delta.class.is_none()
        } else {
            delta.class.is_some_and(|value| (-10..=10).contains(&value))
        }
    }
    fn validate_cursor(&self, cursor: Frame) -> bool {
        cursor.0 <= 32
    }
    fn region(&self, delta: &TrackDelta) -> (Frame, Frame) {
        (delta.first, delta.through)
    }
    fn distance(&self, start: Frame, end: Frame) -> Option<u64> {
        end.0.checked_sub(start.0)
    }
}
pub fn limits() -> RevisionLimits {
    RevisionLimits {
        history_events: 16,
        revisable_units: 4,
    }
}
pub fn reference(sequence: u64, label: &'static str) -> RevisionReference<'static> {
    RevisionReference {
        context: context(),
        sequence,
        event: text(label),
    }
}
pub fn event<'a>(
    domain: &'a Tracking,
    sequence: u64,
    label: &'static str,
    change: RevisionChange<'a, TrackDelta, Frame>,
) -> RevisionEvent<'a, Tracking> {
    RevisionEvent::new(domain, reference(sequence, label), &EVIDENCE, change).unwrap()
}
pub fn delta(first: u64, through: u64, class: i16) -> TrackDelta {
    TrackDelta {
        first: Frame(first),
        through: Frame(through),
        class: Some(class),
    }
}
pub fn reduce(history: &[&RevisionEvent<'_, Tracking>]) -> [Option<i16>; 32] {
    let mut view = [None; 32];
    for event in history {
        let delta = match event.change() {
            RevisionChange::Proposed { delta }
            | RevisionChange::Revised { delta, .. }
            | RevisionChange::Withdrawn { delta, .. }
            | RevisionChange::Corrected { delta, .. } => *delta,
            _ => continue,
        };
        for cell in &mut view[delta.first.0 as usize..delta.through.0 as usize] {
            *cell = delta.class;
        }
    }
    view
}
