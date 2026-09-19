use super::*;

const PROVIDER: NetworkProvider = NetworkProvider {
    base_id: "network/virtio-net-0",
    provider_instance_id: "pci/00:03.0/virtio-net",
    provider_generation: 4,
    boot_id: [7; 32],
};
const ENDPOINT: LiteralIpv4Endpoint = LiteralIpv4Endpoint {
    address: [10, 0, 2, 2],
    port: 443,
};
const BUDGET: WorkBudget = WorkBudget {
    maximum_polls: 32,
    deadline_tick: 100,
};

struct Loopback {
    provider: NetworkProvider,
    frame: [u8; 64],
    len: usize,
    failure: Option<NetworkRefusal>,
    connects: u32,
}

impl Loopback {
    fn new() -> Self {
        Self {
            provider: PROVIDER,
            frame: [0; 64],
            len: 0,
            failure: None,
            connects: 0,
        }
    }

    fn outcome(&mut self) -> Result<(), NetworkRefusal> {
        self.failure.take().map_or(Ok(()), Err)
    }
}

impl OutboundNetworkDriver for Loopback {
    fn provider(&self) -> NetworkProvider {
        self.provider
    }

    fn connect(&mut self, _: LiteralIpv4Endpoint, _: WorkBudget) -> Result<(), NetworkRefusal> {
        self.connects += 1;
        self.outcome()
    }

    fn write(&mut self, bytes: &[u8], _: WorkBudget) -> Result<(), NetworkRefusal> {
        self.outcome()?;
        self.frame[..bytes.len()].copy_from_slice(bytes);
        self.len = bytes.len();
        Ok(())
    }

    fn read(&mut self, output: &mut [u8], _: WorkBudget) -> Result<usize, NetworkRefusal> {
        self.outcome()?;
        output[..self.len].copy_from_slice(&self.frame[..self.len]);
        Ok(self.len)
    }

    fn close(&mut self, _: WorkBudget) -> Result<(), NetworkRefusal> {
        self.outcome()
    }

    fn cancel(&mut self) -> Result<(), NetworkRefusal> {
        self.outcome()
    }
}

#[test]
fn one_session_reuses_fixed_buffers_for_one_hundred_thousand_frames() {
    let mut base = FixedOutboundNetwork::<_, 64, 64>::admit(Loopback::new()).unwrap();
    let session = base.connect(ENDPOINT, BUDGET).unwrap();
    let addresses = (base.rx.as_ptr(), base.tx.as_ptr());
    for sequence in 0_u32..100_000 {
        let frame = sequence.to_le_bytes();
        base.send(session, &frame, BUDGET).unwrap();
        let received = base.receive(session, BUDGET).unwrap();
        assert_eq!(received.as_bytes(), frame);
        drop(received);
    }
    assert_eq!((base.rx.as_ptr(), base.tx.as_ptr()), addresses);
    assert_eq!(base.driver.connects, 1);
    base.close(session, BUDGET).unwrap();
    assert_eq!(base.state(), NetworkState::Closed);
}

#[test]
fn pressure_timeout_cancel_and_loss_remain_distinct() {
    let mut base = FixedOutboundNetwork::<_, 4, 4>::admit(Loopback::new()).unwrap();
    let session = base.connect(ENDPOINT, BUDGET).unwrap();
    assert_eq!(
        base.send(session, &[1, 2, 3, 4, 5], BUDGET),
        Err(NetworkRefusal::Pressure)
    );
    base.driver.failure = Some(NetworkRefusal::Timeout);
    assert_eq!(
        base.send(session, &[1], BUDGET),
        Err(NetworkRefusal::Timeout)
    );
    assert_eq!(base.state(), NetworkState::Open);
    assert_eq!(base.cancel(session), Ok(()));
    assert_eq!(base.state(), NetworkState::Cancelled);
    assert_eq!(
        base.send(session, &[1], BUDGET),
        Err(NetworkRefusal::Cancelled)
    );

    let mut base = FixedOutboundNetwork::<_, 4, 4>::admit(Loopback::new()).unwrap();
    let session = base.connect(ENDPOINT, BUDGET).unwrap();
    base.driver.failure = Some(NetworkRefusal::BaseLost);
    assert!(matches!(
        base.receive(session, BUDGET),
        Err(NetworkRefusal::BaseLost)
    ));
    assert_eq!(base.state(), NetworkState::BaseLost);
}

#[test]
fn stale_provider_and_session_refuse_without_reconnect() {
    let mut base = FixedOutboundNetwork::<_, 8, 8>::admit(Loopback::new()).unwrap();
    let first = base.connect(ENDPOINT, BUDGET).unwrap();
    base.close(first, BUDGET).unwrap();
    let second = base.connect(ENDPOINT, BUDGET).unwrap();
    assert_eq!(
        base.send(first, &[1], BUDGET),
        Err(NetworkRefusal::StaleSession)
    );
    assert_eq!(base.driver.connects, 2);

    base.driver.provider.provider_generation += 1;
    assert_eq!(
        base.send(second, &[1], BUDGET),
        Err(NetworkRefusal::ProviderLost)
    );
    assert_eq!(base.state(), NetworkState::ProviderLost);
    assert_eq!(base.driver.connects, 2);
}

#[test]
fn malformed_endpoint_budget_and_zero_storage_refuse_before_device_use() {
    assert!(matches!(
        FixedOutboundNetwork::<_, 0, 8>::admit(Loopback::new()),
        Err(NetworkRefusal::InvalidProvider)
    ));
    let mut base = FixedOutboundNetwork::<_, 8, 8>::admit(Loopback::new()).unwrap();
    assert_eq!(
        base.connect(
            LiteralIpv4Endpoint {
                address: [0; 4],
                port: 443,
            },
            BUDGET
        ),
        Err(NetworkRefusal::InvalidEndpoint)
    );
    assert_eq!(
        base.connect(
            ENDPOINT,
            WorkBudget {
                maximum_polls: 0,
                deadline_tick: 1,
            }
        ),
        Err(NetworkRefusal::InvalidBudget)
    );
    assert_eq!(base.driver.connects, 0);
}
