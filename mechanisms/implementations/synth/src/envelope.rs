use crate::profile::{micros_to_samples, ReferenceSynthProfile, UNITY_Q16};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EnvelopeStage {
    Attack,
    Decay,
    Sustain,
    Release,
    Idle,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Envelope {
    stage: EnvelopeStage,
    level_q16: u32,
    stage_remaining: u32,
    release_decrement_q16: u32,
}

impl Default for Envelope {
    fn default() -> Self {
        Self {
            stage: EnvelopeStage::Idle,
            level_q16: 0,
            stage_remaining: 0,
            release_decrement_q16: 0,
        }
    }
}

impl Envelope {
    pub fn note_on(&mut self, profile: ReferenceSynthProfile) {
        self.stage = EnvelopeStage::Attack;
        self.level_q16 = 0;
        self.stage_remaining = micros_to_samples(profile.attack_micros);
        if self.stage_remaining == 0 {
            self.enter_decay(profile);
        }
    }

    pub fn note_off(&mut self, profile: ReferenceSynthProfile) {
        if self.stage == EnvelopeStage::Idle {
            return;
        }
        self.stage = EnvelopeStage::Release;
        self.stage_remaining = micros_to_samples(profile.release_micros);
        if self.stage_remaining == 0 {
            self.finish();
        } else {
            self.release_decrement_q16 = self
                .level_q16
                .saturating_add(self.stage_remaining - 1)
                .checked_div(self.stage_remaining)
                .unwrap_or(0);
        }
    }

    pub fn next_level(&mut self, profile: ReferenceSynthProfile) -> u32 {
        match self.stage {
            EnvelopeStage::Attack => {
                let increment = UNITY_Q16.saturating_add(self.stage_remaining - 1)
                    / self.stage_remaining.max(1);
                self.level_q16 = self.level_q16.saturating_add(increment).min(UNITY_Q16);
                self.stage_remaining = self.stage_remaining.saturating_sub(1);
                if self.stage_remaining == 0 || self.level_q16 == UNITY_Q16 {
                    self.enter_decay(profile);
                }
            }
            EnvelopeStage::Decay => {
                let sustain = u32::from(profile.sustain_level_q16);
                let distance = UNITY_Q16.saturating_sub(sustain);
                let total = micros_to_samples(profile.decay_micros).max(1);
                let decrement = distance.saturating_add(total - 1) / total;
                self.level_q16 = self.level_q16.saturating_sub(decrement).max(sustain);
                self.stage_remaining = self.stage_remaining.saturating_sub(1);
                if self.stage_remaining == 0 || self.level_q16 == sustain {
                    self.stage = EnvelopeStage::Sustain;
                    self.level_q16 = sustain;
                }
            }
            EnvelopeStage::Sustain => {}
            EnvelopeStage::Release => {
                self.level_q16 = self.level_q16.saturating_sub(self.release_decrement_q16);
                self.stage_remaining = self.stage_remaining.saturating_sub(1);
                if self.stage_remaining == 0 || self.level_q16 == 0 {
                    self.finish();
                }
            }
            EnvelopeStage::Idle => {}
        }
        self.level_q16
    }

    pub const fn stage(self) -> EnvelopeStage {
        self.stage
    }

    pub const fn level_q16(self) -> u32 {
        self.level_q16
    }

    pub const fn is_idle(self) -> bool {
        matches!(self.stage, EnvelopeStage::Idle)
    }

    fn enter_decay(&mut self, profile: ReferenceSynthProfile) {
        self.stage = EnvelopeStage::Decay;
        self.level_q16 = UNITY_Q16;
        self.stage_remaining = micros_to_samples(profile.decay_micros);
        if self.stage_remaining == 0 {
            self.stage = EnvelopeStage::Sustain;
            self.level_q16 = u32::from(profile.sustain_level_q16);
        }
    }

    fn finish(&mut self) {
        self.stage = EnvelopeStage::Idle;
        self.level_q16 = 0;
        self.stage_remaining = 0;
        self.release_decrement_q16 = 0;
    }
}
