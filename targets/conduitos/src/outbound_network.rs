//! One-session, outbound-only network Base beneath protocol realizations.
//!
//! This module owns finite session and buffer state, not a device driver,
//! resolver, TLS policy, reconnect policy, or portable Line meaning.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkProvider {
    pub base_id: &'static str,
    pub provider_instance_id: &'static str,
    pub provider_generation: u64,
    pub boot_id: [u8; 32],
}

impl NetworkProvider {
    fn is_valid(self) -> bool {
        !self.base_id.is_empty()
            && !self.provider_instance_id.is_empty()
            && self.provider_generation != 0
            && self.boot_id != [0; 32]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiteralIpv4Endpoint {
    pub address: [u8; 4],
    pub port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkBudget {
    pub maximum_polls: u32,
    pub deadline_tick: u64,
}

impl WorkBudget {
    const fn is_valid(self) -> bool {
        self.maximum_polls != 0 && self.deadline_tick != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkState {
    Idle,
    Open,
    Closed,
    Cancelled,
    BaseLost,
    ProviderLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkRefusal {
    InvalidProvider,
    InvalidEndpoint,
    InvalidBudget,
    Pressure,
    Timeout,
    Cancelled,
    Connect,
    Read,
    Write,
    Closed,
    BaseLost,
    ProviderLost,
    StaleSession,
}

impl NetworkRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidProvider => "network-provider-invalid",
            Self::InvalidEndpoint => "network-endpoint-invalid",
            Self::InvalidBudget => "network-work-budget-invalid",
            Self::Pressure => "network-buffer-pressure",
            Self::Timeout => "network-timeout",
            Self::Cancelled => "network-cancelled",
            Self::Connect => "network-connect-failed",
            Self::Read => "network-read-failed",
            Self::Write => "network-write-failed",
            Self::Closed => "network-session-closed",
            Self::BaseLost => "network-base-lost",
            Self::ProviderLost => "network-provider-lost",
            Self::StaleSession => "network-session-stale",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkSession {
    generation: u32,
    endpoint: LiteralIpv4Endpoint,
}

impl NetworkSession {
    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub const fn endpoint(self) -> LiteralIpv4Endpoint {
        self.endpoint
    }
}

/// Exact device/socket realization selected by an Image. Implementations must
/// honor the supplied work budget and must not reconnect internally.
pub trait OutboundNetworkDriver {
    fn provider(&self) -> NetworkProvider;
    fn connect(
        &mut self,
        endpoint: LiteralIpv4Endpoint,
        budget: WorkBudget,
    ) -> Result<(), NetworkRefusal>;
    fn write(&mut self, bytes: &[u8], budget: WorkBudget) -> Result<(), NetworkRefusal>;
    fn read(&mut self, output: &mut [u8], budget: WorkBudget) -> Result<usize, NetworkRefusal>;
    fn close(&mut self, budget: WorkBudget) -> Result<(), NetworkRefusal>;
    fn cancel(&mut self) -> Result<(), NetworkRefusal>;
}

/// One finite socket with Image-selected fixed RX and TX storage.
pub struct FixedOutboundNetwork<D, const RX: usize, const TX: usize> {
    driver: D,
    admitted_provider: NetworkProvider,
    rx: [u8; RX],
    tx: [u8; TX],
    state: NetworkState,
    generation: u32,
    endpoint: Option<LiteralIpv4Endpoint>,
}

impl<D: OutboundNetworkDriver, const RX: usize, const TX: usize> FixedOutboundNetwork<D, RX, TX> {
    pub fn admit(driver: D) -> Result<Self, NetworkRefusal> {
        let provider = driver.provider();
        if !provider.is_valid() || RX == 0 || TX == 0 {
            return Err(NetworkRefusal::InvalidProvider);
        }
        Ok(Self {
            driver,
            admitted_provider: provider,
            rx: [0; RX],
            tx: [0; TX],
            state: NetworkState::Idle,
            generation: 0,
            endpoint: None,
        })
    }

    pub const fn provider(&self) -> NetworkProvider {
        self.admitted_provider
    }

    pub const fn state(&self) -> NetworkState {
        self.state
    }

    pub const fn capacities(&self) -> (usize, usize) {
        (RX, TX)
    }

    pub fn connect(
        &mut self,
        endpoint: LiteralIpv4Endpoint,
        budget: WorkBudget,
    ) -> Result<NetworkSession, NetworkRefusal> {
        self.check_provider()?;
        if endpoint.address == [0; 4] || endpoint.port == 0 {
            return Err(NetworkRefusal::InvalidEndpoint);
        }
        validate_budget(budget)?;
        if !matches!(self.state, NetworkState::Idle | NetworkState::Closed) {
            return Err(NetworkRefusal::Pressure);
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(NetworkRefusal::StaleSession)?;
        match self.driver.connect(endpoint, budget) {
            Ok(()) => {
                self.state = NetworkState::Open;
                self.endpoint = Some(endpoint);
                Ok(NetworkSession {
                    generation: self.generation,
                    endpoint,
                })
            }
            Err(error) => {
                self.transition_for(error);
                Err(error)
            }
        }
    }

    pub fn send(
        &mut self,
        session: NetworkSession,
        bytes: &[u8],
        budget: WorkBudget,
    ) -> Result<(), NetworkRefusal> {
        self.check_open(session)?;
        validate_budget(budget)?;
        if bytes.is_empty() || bytes.len() > TX {
            return Err(NetworkRefusal::Pressure);
        }
        self.tx[..bytes.len()].copy_from_slice(bytes);
        let result = self.driver.write(&self.tx[..bytes.len()], budget);
        self.tx[..bytes.len()].fill(0);
        if let Err(error) = result {
            self.transition_for(error);
        }
        result
    }

    /// Read once into the fixed RX allocation. The returned guard erases the
    /// visible payload when released and cannot outlive this Base borrow.
    pub fn receive(
        &mut self,
        session: NetworkSession,
        budget: WorkBudget,
    ) -> Result<Received<'_>, NetworkRefusal> {
        self.check_open(session)?;
        validate_budget(budget)?;
        self.rx.fill(0);
        match self.driver.read(&mut self.rx, budget) {
            Ok(len) if len == 0 || len > RX => {
                self.rx.fill(0);
                Err(NetworkRefusal::Read)
            }
            Ok(len) => Ok(Received {
                bytes: &mut self.rx[..len],
            }),
            Err(error) => {
                self.rx.fill(0);
                self.transition_for(error);
                Err(error)
            }
        }
    }

    pub fn close(
        &mut self,
        session: NetworkSession,
        budget: WorkBudget,
    ) -> Result<(), NetworkRefusal> {
        self.check_open(session)?;
        validate_budget(budget)?;
        let result = self.driver.close(budget);
        self.rx.fill(0);
        self.tx.fill(0);
        self.state = NetworkState::Closed;
        self.endpoint = None;
        if let Err(error) = result {
            self.transition_for(error);
        }
        result
    }

    pub fn cancel(&mut self, session: NetworkSession) -> Result<(), NetworkRefusal> {
        self.check_open(session)?;
        let result = self.driver.cancel();
        self.rx.fill(0);
        self.tx.fill(0);
        self.state = NetworkState::Cancelled;
        self.endpoint = None;
        if let Err(error) = result {
            self.transition_for(error);
        }
        result
    }

    fn check_open(&mut self, session: NetworkSession) -> Result<(), NetworkRefusal> {
        self.check_provider()?;
        if self.state != NetworkState::Open {
            return Err(match self.state {
                NetworkState::Cancelled => NetworkRefusal::Cancelled,
                NetworkState::BaseLost => NetworkRefusal::BaseLost,
                NetworkState::ProviderLost => NetworkRefusal::ProviderLost,
                _ => NetworkRefusal::Closed,
            });
        }
        if session.generation != self.generation || Some(session.endpoint) != self.endpoint {
            return Err(NetworkRefusal::StaleSession);
        }
        Ok(())
    }

    fn check_provider(&mut self) -> Result<(), NetworkRefusal> {
        if self.driver.provider() != self.admitted_provider {
            self.state = NetworkState::ProviderLost;
            self.endpoint = None;
            self.rx.fill(0);
            self.tx.fill(0);
            return Err(NetworkRefusal::ProviderLost);
        }
        Ok(())
    }

    fn transition_for(&mut self, error: NetworkRefusal) {
        self.state = match error {
            NetworkRefusal::BaseLost => NetworkState::BaseLost,
            NetworkRefusal::ProviderLost => NetworkState::ProviderLost,
            NetworkRefusal::Cancelled => NetworkState::Cancelled,
            _ => self.state,
        };
        if matches!(
            self.state,
            NetworkState::BaseLost | NetworkState::ProviderLost
        ) {
            self.endpoint = None;
        }
    }
}

pub struct Received<'a> {
    bytes: &'a mut [u8],
}

impl Received<'_> {
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes
    }
}

impl Drop for Received<'_> {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

fn validate_budget(budget: WorkBudget) -> Result<(), NetworkRefusal> {
    if budget.is_valid() {
        Ok(())
    } else {
        Err(NetworkRefusal::InvalidBudget)
    }
}

#[cfg(test)]
#[path = "outbound_network/tests.rs"]
mod tests;
