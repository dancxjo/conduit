pub const STATE_BYTES: usize = 13;
pub const COMMAND_BYTES: usize = 3;
pub const MIN_TARGET: i16 = 100;
pub const MAX_TARGET: i16 = 300;
pub const TARGET_STEP: i16 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Mode {
    Off,
    Heat,
    Cool,
    Auto,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Fan {
    Auto,
    On,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Preset {
    Custom,
    Comfort,
    Eco,
    Sleep,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThermostatState {
    pub revision: u32,
    pub target: i16,
    pub mode: Mode,
    pub fan: Fan,
    pub preset: Preset,
    pub measured: Option<i16>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    SetTarget(i16),
    SetMode(Mode),
    SetFan(Fan),
    SetPreset(Preset),
    Observe(Option<i16>),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum Refusal {
    InvalidState = 1,
    InvalidCommand,
    TargetRange,
    ObservationRange,
    RevisionExhausted,
}

impl Default for ThermostatState {
    fn default() -> Self {
        Self {
            revision: 0,
            target: 210,
            mode: Mode::Off,
            fan: Fan::Auto,
            preset: Preset::Comfort,
            measured: None,
        }
    }
}
impl ThermostatState {
    pub fn validate(&self) -> Result<(), Refusal> {
        if !(MIN_TARGET..=MAX_TARGET).contains(&self.target) || self.target % TARGET_STEP != 0 {
            return Err(Refusal::TargetRange);
        }
        if self.measured.is_some_and(|v| !(-500..=800).contains(&v)) {
            return Err(Refusal::ObservationRange);
        }
        let preset_target = preset_target(self.preset, self.mode);
        if preset_target.is_some_and(|v| v != self.target) {
            return Err(Refusal::InvalidState);
        }
        Ok(())
    }
    pub fn apply(&self, command: Command) -> Result<Self, Refusal> {
        self.validate()?;
        let mut next = *self;
        match command {
            Command::SetTarget(v) => {
                next.target = v;
                next.preset = Preset::Custom;
            }
            Command::SetMode(v) => {
                next.mode = v;
                if let Some(target) = preset_target(next.preset, v) {
                    next.target = target;
                }
            }
            Command::SetFan(v) => next.fan = v,
            Command::SetPreset(Preset::Custom) => return Err(Refusal::InvalidCommand),
            Command::SetPreset(v) => {
                next.preset = v;
                next.target = preset_target(v, next.mode).ok_or(Refusal::InvalidCommand)?;
            }
            Command::Observe(v) => next.measured = v,
        }
        next.validate()?;
        if next == *self {
            return Ok(next);
        }
        next.revision = self
            .revision
            .checked_add(1)
            .ok_or(Refusal::RevisionExhausted)?;
        Ok(next)
    }
    pub fn encode(&self) -> Result<[u8; STATE_BYTES], Refusal> {
        self.validate()?;
        let mut out = [0; STATE_BYTES];
        out[0] = 1;
        out[1..5].copy_from_slice(&self.revision.to_le_bytes());
        out[5..7].copy_from_slice(&self.target.to_le_bytes());
        out[7] = self.mode as u8;
        out[8] = self.fan as u8;
        out[9] = self.preset as u8;
        if let Some(v) = self.measured {
            out[10] = 1;
            out[11..13].copy_from_slice(&v.to_le_bytes());
        }
        Ok(out)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Refusal> {
        if bytes.len() != STATE_BYTES || bytes[0] != 1 {
            return Err(Refusal::InvalidState);
        }
        let measured = match bytes[10] {
            0 if bytes[11..13] == [0, 0] => None,
            1 => Some(i16::from_le_bytes([bytes[11], bytes[12]])),
            _ => return Err(Refusal::InvalidState),
        };
        let state = Self {
            revision: u32::from_le_bytes(bytes[1..5].try_into().unwrap()),
            target: i16::from_le_bytes([bytes[5], bytes[6]]),
            mode: mode(bytes[7]).map_err(|_| Refusal::InvalidState)?,
            fan: fan(bytes[8]).map_err(|_| Refusal::InvalidState)?,
            preset: preset(bytes[9]).map_err(|_| Refusal::InvalidState)?,
            measured,
        };
        state.validate()?;
        Ok(state)
    }
    pub fn demand(&self) -> &'static str {
        if self.mode == Mode::Off {
            return "Control is off";
        }
        let Some(current) = self.measured else {
            return "Waiting for a temperature sensor";
        };
        match self.mode {
            Mode::Heat if current < self.target - 5 => "Heating requested",
            Mode::Cool if current > self.target + 5 => "Cooling requested",
            Mode::Auto if current < self.target - 5 => "Heating requested",
            Mode::Auto if current > self.target + 5 => "Cooling requested",
            _ => "Within target deadband",
        }
    }
}
impl Command {
    pub fn encode(self) -> [u8; COMMAND_BYTES] {
        let (tag, value) = match self {
            Self::SetTarget(v) => (0, v),
            Self::SetMode(v) => (1, v as i16),
            Self::SetFan(v) => (2, v as i16),
            Self::SetPreset(v) => (3, v as i16),
            Self::Observe(Some(v)) => (4, v),
            Self::Observe(None) => (5, 0),
        };
        let value = value.to_le_bytes();
        [tag, value[0], value[1]]
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Refusal> {
        if bytes.len() != COMMAND_BYTES {
            return Err(Refusal::InvalidCommand);
        }
        let v = i16::from_le_bytes([bytes[1], bytes[2]]);
        Ok(match bytes[0] {
            0 => Self::SetTarget(v),
            1 if bytes[2] == 0 => Self::SetMode(mode(bytes[1])?),
            2 if bytes[2] == 0 => Self::SetFan(fan(bytes[1])?),
            3 if bytes[2] == 0 && bytes[1] != 0 => Self::SetPreset(preset(bytes[1])?),
            4 => Self::Observe(Some(v)),
            5 if v == 0 => Self::Observe(None),
            _ => return Err(Refusal::InvalidCommand),
        })
    }
}
fn mode(v: u8) -> Result<Mode, Refusal> {
    match v {
        0 => Ok(Mode::Off),
        1 => Ok(Mode::Heat),
        2 => Ok(Mode::Cool),
        3 => Ok(Mode::Auto),
        _ => Err(Refusal::InvalidCommand),
    }
}
fn fan(v: u8) -> Result<Fan, Refusal> {
    match v {
        0 => Ok(Fan::Auto),
        1 => Ok(Fan::On),
        _ => Err(Refusal::InvalidCommand),
    }
}
fn preset(v: u8) -> Result<Preset, Refusal> {
    match v {
        0 => Ok(Preset::Custom),
        1 => Ok(Preset::Comfort),
        2 => Ok(Preset::Eco),
        3 => Ok(Preset::Sleep),
        _ => Err(Refusal::InvalidCommand),
    }
}

fn preset_target(preset: Preset, mode: Mode) -> Option<i16> {
    match (preset, mode) {
        (Preset::Custom, _) => None,
        (Preset::Comfort, Mode::Cool) => Some(240),
        (Preset::Eco, Mode::Cool) => Some(260),
        (Preset::Sleep, Mode::Cool) => Some(250),
        (Preset::Comfort, _) => Some(210),
        (Preset::Eco, _) => Some(180),
        (Preset::Sleep, _) => Some(190),
    }
}
