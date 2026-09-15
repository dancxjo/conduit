//! Presentation-independent Tour chapter and lifecycle state.

pub const TOUR_CHAPTER_COUNT: u8 = 7;
pub const TOUR_RUN_ACTION: &str = "tour.run";
pub const TOUR_STOP_ACTION: &str = "tour.stop";
pub const TOUR_RESTORE_ACTION: &str = "tour.restore";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TourChapter {
    pub identity: &'static str,
    pub route: &'static str,
    pub companion: &'static str,
    pub stages: &'static [TourStage],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TourStage {
    pub identity: &'static str,
    pub mode: TourStageMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourStageMode {
    Run,
    Compare,
    TwoHost,
    TwoHostPlan,
}

impl TourStageMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Compare => "compare",
            Self::TwoHost => "two-host",
            Self::TwoHostPlan => "two-host-plan",
        }
    }
}

const CHAPTER_ONE_STAGES: [TourStage; 3] = [
    TourStage {
        identity: "canonical-form:meet-one-gear",
        mode: TourStageMode::Run,
    },
    TourStage {
        identity: "canonical-form:edit-one-gear",
        mode: TourStageMode::Run,
    },
    TourStage {
        identity: "canonical-form:branch-a-cord",
        mode: TourStageMode::Run,
    },
];
const CHAPTER_TWO_STAGES: [TourStage; 1] = [TourStage {
    identity: "canonical-form:same-morse-caller",
    mode: TourStageMode::Compare,
}];
const CHAPTER_THREE_STAGES: [TourStage; 1] = [TourStage {
    identity: "canonical-form:count-over-time",
    mode: TourStageMode::Run,
}];
const CHAPTER_FOUR_STAGES: [TourStage; 2] = [
    TourStage {
        identity: "canonical-form:hello-across",
        mode: TourStageMode::TwoHost,
    },
    TourStage {
        identity: "canonical-form:hello-across",
        mode: TourStageMode::TwoHostPlan,
    },
];
const NO_STAGES: [TourStage; 0] = [];

pub const TOUR_CHAPTERS: [TourChapter; TOUR_CHAPTER_COUNT as usize] = [
    TourChapter {
        identity: "form-basics",
        route: "one-program-many-computers",
        companion: "form-laboratory",
        stages: &CHAPTER_ONE_STAGES,
    },
    TourChapter {
        identity: "faces-and-backs",
        route: "faces-backs-and-implementation",
        companion: "recursive-form",
        stages: &CHAPTER_TWO_STAGES,
    },
    TourChapter {
        identity: "host-realization",
        route: "hosts-make-forms-real",
        companion: "host-inventory",
        stages: &CHAPTER_THREE_STAGES,
    },
    TourChapter {
        identity: "multi-host-form",
        route: "one-form-across-several-hosts",
        companion: "multi-host-plan",
        stages: &CHAPTER_FOUR_STAGES,
    },
    TourChapter {
        identity: "body-continuity",
        route: "the-body-one-computer-one-machine-or-many",
        companion: "body-continuity",
        stages: &NO_STAGES,
    },
    TourChapter {
        identity: "body-wide-realization",
        route: "many-forms-one-body-wide-realization",
        companion: "body-workload",
        stages: &NO_STAGES,
    },
    TourChapter {
        identity: "birth-and-spores",
        route: "birth-spores-and-the-creche",
        companion: "creche-handoff",
        stages: &NO_STAGES,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TourRunState {
    Ready = 0,
    Running = 1,
    Stopped = 2,
    Completed = 3,
    Failed = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TourProgressState {
    pub chapter: u8,
    pub chapter_count: u8,
    pub run: TourRunState,
    pub source_is_canonical: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TourApplicationState {
    pub revision: u32,
    pub progress: TourProgressState,
}

impl core::ops::Deref for TourApplicationState {
    type Target = TourProgressState;

    fn deref(&self) -> &Self::Target {
        &self.progress
    }
}

impl core::ops::DerefMut for TourApplicationState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.progress
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourApplicationAction {
    OpenChapter(u8),
    PreviousChapter,
    NextChapter,
    Run,
    Stop,
    Restore,
    Complete,
    Fail,
}

pub const TOUR_PROJECTION_CONFORMANCE_ACTIONS: [TourApplicationAction; 6] = [
    TourApplicationAction::NextChapter,
    TourApplicationAction::Run,
    TourApplicationAction::Stop,
    TourApplicationAction::Restore,
    TourApplicationAction::PreviousChapter,
    TourApplicationAction::Run,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourApplicationRefusal {
    InvalidChapterCount,
    FirstChapter,
    LastChapter,
    AlreadyRunning,
    NotRunning,
    RevisionExhausted,
}

impl TourApplicationState {
    pub const fn canonical() -> Self {
        Self {
            revision: 1,
            progress: TourProgressState::canonical(),
        }
    }

    pub const fn with_chapter_count(chapter_count: u8) -> Result<Self, TourApplicationRefusal> {
        if chapter_count != TOUR_CHAPTER_COUNT {
            return Err(TourApplicationRefusal::InvalidChapterCount);
        }
        Ok(Self {
            progress: TourProgressState {
                chapter_count,
                ..TourProgressState::canonical()
            },
            ..Self::canonical()
        })
    }

    pub fn apply(&mut self, action: TourApplicationAction) -> Result<(), TourApplicationRefusal> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(TourApplicationRefusal::RevisionExhausted)?;
        match action {
            TourApplicationAction::OpenChapter(chapter) if chapter >= self.chapter_count => {
                return Err(TourApplicationRefusal::LastChapter);
            }
            TourApplicationAction::OpenChapter(chapter) => self.chapter = chapter,
            TourApplicationAction::PreviousChapter if self.chapter == 0 => {
                return Err(TourApplicationRefusal::FirstChapter);
            }
            TourApplicationAction::PreviousChapter => self.chapter -= 1,
            TourApplicationAction::NextChapter if self.chapter + 1 >= self.chapter_count => {
                return Err(TourApplicationRefusal::LastChapter);
            }
            TourApplicationAction::NextChapter => self.chapter += 1,
            TourApplicationAction::Run if self.run == TourRunState::Running => {
                return Err(TourApplicationRefusal::AlreadyRunning);
            }
            TourApplicationAction::Run => self.run = TourRunState::Running,
            TourApplicationAction::Stop if self.run != TourRunState::Running => {
                return Err(TourApplicationRefusal::NotRunning);
            }
            TourApplicationAction::Stop => self.run = TourRunState::Stopped,
            TourApplicationAction::Restore => {
                self.source_is_canonical = true;
                if self.run == TourRunState::Running {
                    self.run = TourRunState::Stopped;
                }
            }
            TourApplicationAction::Complete if self.run != TourRunState::Running => {
                return Err(TourApplicationRefusal::NotRunning);
            }
            TourApplicationAction::Complete => self.run = TourRunState::Completed,
            TourApplicationAction::Fail if self.run != TourRunState::Running => {
                return Err(TourApplicationRefusal::NotRunning);
            }
            TourApplicationAction::Fail => self.run = TourRunState::Failed,
        }
        self.revision = revision;
        Ok(())
    }

    pub fn mark_source_edited(&mut self) -> Result<(), TourApplicationRefusal> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(TourApplicationRefusal::RevisionExhausted)?;
        self.source_is_canonical = false;
        Ok(())
    }
}

impl TourProgressState {
    pub const fn canonical() -> Self {
        Self {
            chapter: 0,
            chapter_count: TOUR_CHAPTER_COUNT,
            run: TourRunState::Ready,
            source_is_canonical: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representative_application_trace_is_exact_and_finite() {
        let mut state = TourApplicationState::canonical();
        for action in [
            TourApplicationAction::NextChapter,
            TourApplicationAction::Run,
            TourApplicationAction::Stop,
        ] {
            state.apply(action).unwrap();
        }
        state.mark_source_edited().unwrap();
        state.apply(TourApplicationAction::Restore).unwrap();
        state.apply(TourApplicationAction::PreviousChapter).unwrap();
        assert_eq!(state.chapter, 0);
        assert_eq!(state.run, TourRunState::Stopped);
        assert!(state.source_is_canonical);
        assert_eq!(state.revision, 7);
    }

    #[test]
    fn refused_actions_do_not_mutate_application_state() {
        let mut state = TourApplicationState::canonical();
        let before = state;
        assert_eq!(
            state.apply(TourApplicationAction::PreviousChapter),
            Err(TourApplicationRefusal::FirstChapter)
        );
        assert_eq!(state, before);
        assert_eq!(
            state.apply(TourApplicationAction::Stop),
            Err(TourApplicationRefusal::NotRunning)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn canonical_catalog_owns_every_executable_stage_identity_and_mode() {
        let stages = TOUR_CHAPTERS
            .iter()
            .flat_map(|chapter| chapter.stages)
            .collect::<alloc::vec::Vec<_>>();
        assert_eq!(stages.len(), 7);
        assert_eq!(TOUR_CHAPTERS[0].stages.len(), 3);
        assert_eq!(TOUR_CHAPTERS[1].stages.len(), 1);
        assert_eq!(TOUR_CHAPTERS[2].stages.len(), 1);
        assert_eq!(TOUR_CHAPTERS[3].stages.len(), 2);
        assert!(
            TOUR_CHAPTERS[4..]
                .iter()
                .all(|chapter| chapter.stages.is_empty())
        );
        assert_eq!(stages[0].identity, "canonical-form:meet-one-gear");
        assert_eq!(stages[6].mode, TourStageMode::TwoHostPlan);
    }
}
