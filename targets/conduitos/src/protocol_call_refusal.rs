//! Stable machine-readable refusals for native protocol Host Calls.
use crate::{
    expression_host_call::ExpressionCallRefusal, i2c_base::owner::I2cCallRefusal,
    structured_selector_host_call::SelectorCallRefusal,
};
use conduit_composite::KernelCompositeError;
use conduit_kernel::{Failure, FailureCode};

#[derive(Debug)]
pub enum ProtocolCallRefusal {
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    Usb(crate::arch::UsbControlCallRefusal),
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    UsbProtocol(crate::arch::UsbNativeProtocolRefusal),
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    UsbIsochronous(crate::arch::UsbIsochronousCallRefusal),
    #[cfg(feature = "native-owner-network")]
    EthernetFrame(crate::ethernet_frame::installation::EthernetFrameNativeRefusal),
    Storage(crate::storage_base::transaction_owner::StorageTransactionRefusal),
    InvalidPlan,
    Unsupported,
    Kernel(KernelCompositeError),
    Expression(ExpressionCallRefusal),
    Selector(SelectorCallRefusal),
    I2c(I2cCallRefusal),
    Clock(crate::monotonic_clock::owner::ClockCallRefusal),
}

impl ProtocolCallRefusal {
    pub fn failure(&self) -> Failure {
        use crate::monotonic_clock::owner::ClockCallRefusal as Clock;
        use ExpressionCallRefusal as Expression;
        use I2cCallRefusal as I2c;
        use SelectorCallRefusal as Selector;
        let (code, detail) = match self {
            #[cfg(feature = "native-owner-network")]
            Self::EthernetFrame(error) => {
                use crate::ethernet_frame::installation::EthernetFrameNativeRefusal as E;
                match error {
                    E::Identity | E::Advertisement | E::Binding | E::StaleRequest => {
                        (FailureCode::InvalidLifecycle, 1210)
                    }
                    E::Possession | E::Capability(_) => (FailureCode::HostCallDenied, 1211),
                    E::Canonical(_) => (FailureCode::InvalidInput, 1212),
                    E::Network(_) => (FailureCode::HostCallFailed, 1213),
                }
            }
            Self::Storage(error) => {
                use crate::storage_base::transaction_owner::StorageTransactionRefusal as S;
                let code = match error {
                    S::Binding | S::Stale | S::Terminal => FailureCode::InvalidLifecycle,
                    S::Possession => FailureCode::HostCallDenied,
                    S::Pressure => FailureCode::StorageExhausted,
                    S::Candidate | S::Canonical(_) | S::Request(_) => FailureCode::InvalidInput,
                    S::Capability(_) => FailureCode::HostCallFailed,
                };
                (code, 1097)
            }

            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            Self::Usb(error) => control_failure(error),
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            Self::UsbProtocol(error) => {
                use crate::arch::{UsbNativeHostRefusal as Host, UsbNativeProtocolRefusal as Usb};
                match error {
                    Usb::Installation(_) | Usb::EndpointOffer(_) => {
                        (FailureCode::InvalidLifecycle, 1080)
                    }
                    Usb::Canonical(_) => (FailureCode::InvalidInput, 1081),
                    Usb::Control(error) | Usb::Host(Host::Control(error)) => control_failure(error),
                    Usb::Endpoint(error) | Usb::Host(Host::Endpoint(error)) => {
                        endpoint_failure(error)
                    }
                    Usb::Write(error) | Usb::Host(Host::Write(error)) => write_failure(error),
                    Usb::Host(Host::Binding | Host::NotReady) => {
                        (FailureCode::InvalidLifecycle, 1090)
                    }
                    Usb::Host(Host::Isochronous(_)) => (FailureCode::HostCallFailed, 1093),
                    Usb::Host(Host::Capacity) => (FailureCode::IdentityCapacityExhausted, 1091),
                    Usb::Host(Host::Native(error)) => native_usb_failure(error, 1092),
                    Usb::Host(Host::Pending) => (FailureCode::HostCallFailed, 1092),
                }
            }
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            Self::UsbIsochronous(_) => (FailureCode::HostCallFailed, 1094),
            Self::InvalidPlan => (FailureCode::InvalidLifecycle, 1020),
            Self::Unsupported => (FailureCode::HostCallDenied, 1021),
            Self::Kernel(_) => (FailureCode::HostCallFailed, 1022),
            Self::Expression(
                Expression::WrongBinding | Expression::StaleRequest | Expression::InvalidProgram,
            ) => (FailureCode::InvalidLifecycle, 1023),
            Self::Expression(Expression::InvalidInput) => (FailureCode::InvalidInput, 1024),
            Self::Expression(Expression::SequenceExhausted) => {
                (FailureCode::IdentityCapacityExhausted, 1025)
            }
            Self::Expression(Expression::Cancelled) => (FailureCode::Cancelled, 1026),
            Self::Expression(Expression::Evaluation(_)) => (FailureCode::HostCallFailed, 1027),
            Self::Selector(
                Selector::WrongBinding | Selector::InvalidSelector | Selector::StaleRequest,
            ) => (FailureCode::InvalidLifecycle, 1040),
            Self::Selector(Selector::InvalidInput) => (FailureCode::InvalidInput, 1041),
            Self::Selector(Selector::SequenceExhausted) => {
                (FailureCode::IdentityCapacityExhausted, 1042)
            }
            Self::Selector(Selector::Cancelled) => (FailureCode::Cancelled, 1043),
            Self::Selector(Selector::Selection(_)) => (FailureCode::InvalidInput, 1044),
            Self::Clock(Clock::WrongBinding | Clock::StaleRequest | Clock::Pending) => {
                (FailureCode::InvalidLifecycle, 1060)
            }
            Self::Clock(Clock::Possession | Clock::Capability(_)) => {
                (FailureCode::HostCallDenied, 1061)
            }
            Self::Clock(Clock::SequenceExhausted) => (FailureCode::IdentityCapacityExhausted, 1062),
            Self::Clock(Clock::Cancelled) => (FailureCode::Cancelled, 1063),
            Self::Clock(Clock::Canonical(_)) => (FailureCode::InvalidInput, 1064),
            Self::Clock(Clock::Temporal(_)) => (FailureCode::InvalidInput, 1065),
            Self::Clock(Clock::Provider(_)) => (FailureCode::HostCallFailed, 1066),
            Self::I2c(I2c::WrongBinding | I2c::StaleRequest) => {
                (FailureCode::InvalidLifecycle, 1030)
            }
            Self::I2c(I2c::Possession | I2c::Capability(_)) => (FailureCode::HostCallDenied, 1031),
            Self::I2c(I2c::SequenceExhausted) => (FailureCode::IdentityCapacityExhausted, 1032),
            Self::I2c(I2c::Decode(_)) => (FailureCode::InvalidInput, 1033),
            Self::I2c(I2c::Result(_) | I2c::Canonical(_)) => (FailureCode::HostCallFailed, 1034),
        };
        Failure { code, detail }
    }
}

#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
))]
fn control_failure(error: &crate::arch::UsbControlCallRefusal) -> (FailureCode, u16) {
    use crate::arch::UsbControlCallRefusal as Usb;
    use crate::usb_base::control_owner::ControlOwnerRefusal as Owner;
    match error {
        Usb::Ownership(Owner::WrongBinding | Owner::StaleTransfer) => {
            (FailureCode::InvalidLifecycle, 1070)
        }
        Usb::Ownership(Owner::Possession | Owner::Capability(_)) => {
            (FailureCode::HostCallDenied, 1071)
        }
        Usb::Ownership(Owner::Pressure) => (FailureCode::HostCallFailed, 1072),
        Usb::Ownership(Owner::SequenceExhausted) => (FailureCode::IdentityCapacityExhausted, 1073),
        Usb::Ownership(Owner::Decode(_)) => (FailureCode::InvalidInput, 1074),
        Usb::Ownership(Owner::Result(_) | Owner::Canonical(_)) => {
            (FailureCode::HostCallFailed, 1075)
        }
        Usb::Native(error) => native_usb_failure(error, 1076),
    }
}

#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
))]
fn native_usb_failure(
    error: &crate::arch::usb_xhci::UsbError,
    fallback: u16,
) -> (FailureCode, u16) {
    use crate::arch::usb_xhci::UsbError;
    match error {
        UsbError::StaleDeviceInstance => (FailureCode::InvalidLifecycle, 1095),
        UsbError::DeviceVanished => (FailureCode::HostCallFailed, 1096),
        _ => (FailureCode::HostCallFailed, fallback),
    }
}

#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
))]
fn endpoint_failure(error: &crate::arch::EndpointNativeRefusal) -> (FailureCode, u16) {
    use crate::arch::EndpointNativeRefusal as Endpoint;
    use crate::usb_base::endpoint_read_owner::EndpointReadOwnerRefusal as Owner;
    match error {
        Endpoint::Ownership(Owner::WrongBinding | Owner::StaleTransfer) => {
            (FailureCode::InvalidLifecycle, 1083)
        }
        Endpoint::Ownership(Owner::Possession | Owner::Capability(_)) => {
            (FailureCode::HostCallDenied, 1084)
        }
        Endpoint::Ownership(Owner::SequenceExhausted) => {
            (FailureCode::IdentityCapacityExhausted, 1085)
        }
        Endpoint::Ownership(Owner::Decode(_)) => (FailureCode::InvalidInput, 1086),
        Endpoint::Ownership(Owner::Result(_) | Owner::Canonical(_) | Owner::Pressure) => {
            (FailureCode::HostCallFailed, 1087)
        }
        Endpoint::Mapping | Endpoint::ForeignCompletion => (FailureCode::InvalidLifecycle, 1088),
        Endpoint::Ring(_)
        | Endpoint::Pending
        | Endpoint::Completion(_)
        | Endpoint::ProviderLost
        | Endpoint::Stop => (FailureCode::HostCallFailed, 1089),
    }
}

#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
))]
fn write_failure(error: &crate::arch::EndpointWriteNativeRefusal) -> (FailureCode, u16) {
    use crate::arch::EndpointWriteNativeRefusal as Endpoint;
    use crate::usb_base::endpoint_write_owner::EndpointWriteOwnerRefusal as Owner;
    match error {
        Endpoint::Ownership(Owner::WrongBinding | Owner::StaleTransfer) => {
            (FailureCode::InvalidLifecycle, 1183)
        }
        Endpoint::Ownership(Owner::Possession | Owner::Capability(_)) => {
            (FailureCode::HostCallDenied, 1184)
        }
        Endpoint::Ownership(Owner::SequenceExhausted) => {
            (FailureCode::IdentityCapacityExhausted, 1185)
        }
        Endpoint::Ownership(Owner::Decode(_)) => (FailureCode::InvalidInput, 1186),
        Endpoint::Ownership(Owner::Result(_) | Owner::Canonical(_) | Owner::Pressure) => {
            (FailureCode::HostCallFailed, 1187)
        }
        Endpoint::Mapping | Endpoint::ForeignCompletion => (FailureCode::InvalidLifecycle, 1188),
        Endpoint::Ring(_)
        | Endpoint::Pending
        | Endpoint::Completion(_)
        | Endpoint::ProviderLost
        | Endpoint::Stop => (FailureCode::HostCallFailed, 1189),
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;
    use crate::arch::{
        EndpointNativeRefusal as Endpoint, UsbControlCallRefusal as Control,
        UsbNativeHostRefusal as Host, UsbNativeProtocolRefusal as Usb,
    };
    use crate::usb_base::{
        control_decode::ControlDecodeRefusal, control_owner::ControlOwnerRefusal as ControlOwner,
        control_request::ControlRequestRefusal,
        endpoint_read_owner::EndpointReadOwnerRefusal as EndpointOwner,
        endpoint_read_request::EndpointReadRequestRefusal,
    };

    #[test]
    fn control_refusal_categories_survive_native_host_nesting() {
        for (make_error, expected) in [
            (0, FailureCode::InvalidLifecycle),
            (1, FailureCode::HostCallDenied),
            (2, FailureCode::InvalidInput),
        ] {
            let error = || {
                Control::Ownership(match make_error {
                    0 => ControlOwner::WrongBinding,
                    1 => ControlOwner::Possession,
                    _ => ControlOwner::Decode(ControlDecodeRefusal::Transfer(
                        ControlRequestRefusal::DataEnvelope,
                    )),
                })
            };
            let direct = ProtocolCallRefusal::Usb(error()).failure();
            assert_eq!(direct.code, expected);
            assert_eq!(
                ProtocolCallRefusal::UsbProtocol(Usb::Control(error())).failure(),
                direct
            );
            assert_eq!(
                ProtocolCallRefusal::UsbProtocol(Usb::Host(Host::Control(error()))).failure(),
                direct
            );
        }
    }

    #[test]
    fn endpoint_refusal_categories_survive_native_host_nesting() {
        for (make_error, expected) in [
            (0, FailureCode::InvalidLifecycle),
            (1, FailureCode::HostCallDenied),
            (2, FailureCode::InvalidInput),
        ] {
            let error = || {
                Endpoint::Ownership(match make_error {
                    0 => EndpointOwner::WrongBinding,
                    1 => EndpointOwner::Possession,
                    _ => EndpointOwner::Decode(EndpointReadRequestRefusal::Length),
                })
            };
            let direct = ProtocolCallRefusal::UsbProtocol(Usb::Endpoint(error())).failure();
            assert_eq!(direct.code, expected);
            assert_eq!(
                ProtocolCallRefusal::UsbProtocol(Usb::Host(Host::Endpoint(error()))).failure(),
                direct
            );
        }
    }
    #[test]
    fn write_refusal_categories_survive_native_host_nesting() {
        use crate::arch::EndpointWriteNativeRefusal as Write;
        use crate::usb_base::{
            endpoint_write_owner::EndpointWriteOwnerRefusal as Owner,
            endpoint_write_request::EndpointWriteRequestRefusal as Request,
        };
        for (category, expected) in [
            (0, FailureCode::InvalidLifecycle),
            (1, FailureCode::HostCallDenied),
            (2, FailureCode::InvalidInput),
        ] {
            let error = || {
                Write::Ownership(match category {
                    0 => Owner::WrongBinding,
                    1 => Owner::Possession,
                    _ => Owner::Decode(Request::Length),
                })
            };
            let direct = ProtocolCallRefusal::UsbProtocol(Usb::Write(error())).failure();
            assert_eq!(direct.code, expected);
            assert_eq!(
                ProtocolCallRefusal::UsbProtocol(Usb::Host(Host::Write(error()))).failure(),
                direct
            );
        }
    }
    #[test]
    fn stale_attachment_and_provider_loss_keep_distinct_native_failure_identity() {
        use crate::arch::usb_xhci::UsbError;
        for (stale, code, detail) in [
            (true, FailureCode::InvalidLifecycle, 1095),
            (false, FailureCode::HostCallFailed, 1096),
        ] {
            let error = || {
                if stale {
                    UsbError::StaleDeviceInstance
                } else {
                    UsbError::DeviceVanished
                }
            };
            let direct = ProtocolCallRefusal::Usb(Control::Native(error())).failure();
            assert_eq!(direct, Failure { code, detail });
            assert_eq!(
                ProtocolCallRefusal::UsbProtocol(Usb::Host(Host::Native(error()))).failure(),
                direct
            );
            assert_eq!(
                ProtocolCallRefusal::UsbProtocol(Usb::Host(Host::Control(
                    Control::Native(error())
                )))
                .failure(),
                direct
            );
        }
    }
}
