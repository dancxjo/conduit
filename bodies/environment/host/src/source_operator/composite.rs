//! Four finite source slots sharing one actual installed collector Host owner.
use super::network::NetworkSource;
use super::*;
use conduit_environment_runtime::environmental_reading::source_observation::SourceObservation;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollectorSourceId(usize);
enum Provider {
    Usb(DeviceEnvironmentalSource),
    Network(NetworkSource),
}
impl EnvironmentalSourceProvider for Provider {
    async fn observe(
        &mut self,
    ) -> Result<SourceObservation, conduit_environment_runtime::environmental_source::Refusal> {
        match self {
            Self::Usb(p) => p.observe().await,
            Self::Network(p) => p.observe().await,
        }
    }
}
pub struct CollectorRoot {
    installed: SharedHost,
    slots: Vec<UsbSourceRoot<Provider>>,
}
impl CollectorRoot {
    pub fn new(installed: conduit_std_host::StdHost) -> Self {
        Self {
            installed: Rc::new(RefCell::new(Some(installed))),
            slots: Vec::with_capacity(4),
        }
    }
    pub fn attach_usb(
        &mut self,
        path: &Path,
        configuration: &CheckedHostConfiguration,
        firmware: [u8; 32],
        budget: Duration,
    ) -> Result<CollectorSourceId, Refusal> {
        self.capacity()?;
        let provider = {
            let owner = self.installed.borrow();
            let host = owner.as_ref().ok_or(Refusal::Acquisition)?;
            DeviceEnvironmentalSource::acquire(
                path,
                host.advertisement(),
                configuration,
                firmware,
                budget,
            )
            .map_err(|_| Refusal::Acquisition)?
        };
        self.attach(Provider::Usb(provider))
    }
    pub fn attach_network(
        &mut self,
        network: NetworkSourceConfiguration,
        configuration: &CheckedHostConfiguration,
        budget: Duration,
    ) -> Result<CollectorSourceId, Refusal> {
        self.capacity()?;
        let provider =
            NetworkSource::acquire(self.installed.clone(), network, configuration, budget)?;
        self.attach(Provider::Network(provider))
    }
    fn capacity(&self) -> Result<(), Refusal> {
        if self.slots.len() >= 4 {
            Err(Refusal::Capacity)
        } else {
            Ok(())
        }
    }
    fn attach(&mut self, provider: Provider) -> Result<CollectorSourceId, Refusal> {
        let realization = match &provider {
            Provider::Usb(_) => "usb",
            Provider::Network(_) => "isolated-http",
        };
        let root = UsbSourceRoot::attach(self.installed.clone(), provider, realization)?;
        let id = CollectorSourceId(self.slots.len());
        self.slots.push(root);
        Ok(id)
    }
    /// Serial capture is enforced by exclusive access to the same composite owner.
    pub async fn sample(&mut self, id: CollectorSourceId) -> Result<CommittedSample, Refusal> {
        self.slots
            .get_mut(id.0)
            .ok_or(Refusal::Possession)?
            .sample()
            .await
    }
}
