//! Attended AP/HTTP bench. No native Host/Body installation is asserted.
use embassy_executor::Spawner;
use embassy_net::{Config, Ipv4Address, Ipv4Cidr, StackResources, StaticConfigV4};
use esp_radio::wifi::{ap::AccessPointConfig, Config as WifiConfig, ControllerConfig, CountryInfo};
use static_cell::StaticCell;
static RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();
#[embassy_executor::task]
async fn runner(mut runner: embassy_net::Runner<'static, esp_radio::wifi::Interface<'static>>) {
    runner.run().await
}
pub fn start(
    spawner: Spawner,
    wifi: esp_hal::peripherals::WIFI<'static>,
    seed: u64,
) -> esp_radio::wifi::WifiController<'static> {
    let ap = AccessPointConfig::default()
        .with_ssid("Conduit-Climate")
        .with_max_connections(2);
    let config = ControllerConfig::default()
        .with_country_info(CountryInfo::from(*b"US"))
        .with_rx_queue_size(3)
        .with_tx_queue_size(2)
        .with_static_rx_buf_num(4)
        .with_dynamic_rx_buf_num(8)
        .with_dynamic_tx_buf_num(4)
        .with_ampdu_rx_enable(false)
        .with_ampdu_tx_enable(false)
        .with_initial_config(WifiConfig::AccessPoint(ap));
    let (controller, interfaces) = esp_radio::wifi::new(wifi, config).unwrap();
    let config = Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 4, 1), 24),
        gateway: None,
        dns_servers: Default::default(),
    });
    let (stack, net_runner) = embassy_net::new(
        interfaces.access_point,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );
    spawner.spawn(runner(net_runner).unwrap());
    spawner.spawn(crate::services::run(stack).unwrap());
    controller
}
