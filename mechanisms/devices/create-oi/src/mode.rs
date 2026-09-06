//! Finite stop-first Create OI mode transition shared by every UART Host.

use crate::{
    encode_mode, encode_query_sensor, encode_start, encode_stop, read_query_sensor_packet,
    write_command, CreateOiFailure, CreateOiModeRequest, CreateUartProvider,
};

pub const CREATE_OI_MODE_PACKET_ID: u8 = 35;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CreateOiModeObservation {
    Off,
    Passive,
    Safe,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateOiModeTransitionStage {
    MandatoryStop,
    ModeTransition,
    VerificationQuery,
    VerificationRead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateOiModeTransitionFailure {
    pub stage: CreateOiModeTransitionStage,
    pub failure: CreateOiFailure,
}

pub fn transition_oi_mode<P: CreateUartProvider>(
    provider: &mut P,
    target: CreateOiModeRequest,
    deadline_tick: u64,
) -> Result<CreateOiModeObservation, CreateOiModeTransitionFailure> {
    write_command(provider, &encode_stop()).map_err(|failure| CreateOiModeTransitionFailure {
        stage: CreateOiModeTransitionStage::MandatoryStop,
        failure,
    })?;
    // Create ignores every OI command other than Start while its OI is Off.
    // The leading stop is therefore effective when an earlier controller left
    // the OI controllable, while Start is always required before selecting the
    // requested mode on a newly powered Create.
    write_command(provider, &encode_start()).map_err(|failure| CreateOiModeTransitionFailure {
        stage: CreateOiModeTransitionStage::ModeTransition,
        failure,
    })?;
    if target != CreateOiModeRequest::Passive {
        let transition = encode_mode(target).expect("Safe and Full have exact mode commands");
        write_command(provider, &transition).map_err(|failure| CreateOiModeTransitionFailure {
            stage: CreateOiModeTransitionStage::ModeTransition,
            failure,
        })?;
        // Start deliberately passes through Passive, where Drive is ignored.
        // Repeat the stop after Safe or Full becomes active so the transition
        // has an effective zero-motion disposition in both starting states.
        write_command(provider, &encode_stop()).map_err(|failure| {
            CreateOiModeTransitionFailure {
                stage: CreateOiModeTransitionStage::MandatoryStop,
                failure,
            }
        })?;
    }
    let query = encode_query_sensor(CREATE_OI_MODE_PACKET_ID)
        .expect("Create OI mode packet is allow-listed");
    write_command(provider, &query).map_err(|failure| CreateOiModeTransitionFailure {
        stage: CreateOiModeTransitionStage::VerificationQuery,
        failure,
    })?;
    let packet = read_query_sensor_packet(provider, CREATE_OI_MODE_PACKET_ID, deadline_tick)
        .map_err(|failure| CreateOiModeTransitionFailure {
            stage: CreateOiModeTransitionStage::VerificationRead,
            failure,
        })?;
    Ok(match packet.bytes()[0] {
        0 => CreateOiModeObservation::Off,
        1 => CreateOiModeObservation::Passive,
        2 => CreateOiModeObservation::Safe,
        3 => CreateOiModeObservation::Full,
        _ => unreachable!("validated packet 35 has one exact OI mode"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UartProfile;
    use std::{collections::VecDeque, vec, vec::Vec};

    struct Provider {
        writes: Vec<Vec<u8>>,
        read: VecDeque<u8>,
    }

    impl CreateUartProvider for Provider {
        type Error = ();

        fn is_available(&self) -> bool {
            true
        }

        fn profile(&self) -> UartProfile {
            UartProfile::CREATE_OI
        }

        fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
            self.writes.push(bytes.to_vec());
            Ok(())
        }

        fn read_byte(&mut self, _: u64) -> Result<Option<u8>, Self::Error> {
            Ok(self.read.pop_front())
        }
    }

    #[test]
    fn all_modes_start_oi_and_use_one_exact_stop_first_transaction() {
        for (target, observed, expected) in [
            (
                CreateOiModeRequest::Passive,
                CreateOiModeObservation::Passive,
                vec![vec![145, 0, 0, 0, 0], vec![128], vec![142, 35]],
            ),
            (
                CreateOiModeRequest::Safe,
                CreateOiModeObservation::Safe,
                vec![
                    vec![145, 0, 0, 0, 0],
                    vec![128],
                    vec![131],
                    vec![145, 0, 0, 0, 0],
                    vec![142, 35],
                ],
            ),
            (
                CreateOiModeRequest::Full,
                CreateOiModeObservation::Full,
                vec![
                    vec![145, 0, 0, 0, 0],
                    vec![128],
                    vec![132],
                    vec![145, 0, 0, 0, 0],
                    vec![142, 35],
                ],
            ),
        ] {
            let mut provider = Provider {
                writes: Vec::new(),
                read: VecDeque::from([observed as u8]),
            };
            assert_eq!(transition_oi_mode(&mut provider, target, 100), Ok(observed));
            assert_eq!(provider.writes, expected);
        }
    }
}
