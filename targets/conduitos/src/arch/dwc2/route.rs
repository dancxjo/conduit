//! Root-retained bus routing geometry. None of these values grants possession.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferType {
    Control,
    Interrupt,
}
impl TransferType {
    pub(super) const fn bits(self) -> u32 {
        match self {
            Self::Control => 0,
            Self::Interrupt => 3,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransactionTranslator {
    pub hub_address: u8,
    pub port: u8,
    pub low_speed: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EndpointRoute {
    pub address: u8,
    pub endpoint: u8,
    pub maximum_packet: u8,
    pub transfer_type: TransferType,
    pub translator: Option<TransactionTranslator>,
}
impl EndpointRoute {
    pub const fn control(address: u8, maximum_packet: u8) -> Self {
        Self {
            address,
            endpoint: 0,
            maximum_packet,
            transfer_type: TransferType::Control,
            translator: None,
        }
    }
    pub(crate) fn valid(self) -> bool {
        self.address <= 127
            && self.endpoint <= 15
            && self.maximum_packet > 0
            && self.maximum_packet <= 64
            && (self.transfer_type != TransferType::Control || self.endpoint == 0)
            && (self.transfer_type == TransferType::Control || self.endpoint != 0)
            && self.translator.is_none_or(|tt| {
                tt.hub_address > 0
                    && tt.hub_address <= 127
                    && tt.port > 0
                    && tt.port <= 127
                    && (!tt.low_speed || self.maximum_packet <= 8)
            })
    }
}
