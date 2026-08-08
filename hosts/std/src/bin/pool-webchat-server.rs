fn main() -> Result<(), String> {
    let bind = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:0".to_string());
    conduit_std_host::pool_webchat::run(&bind)
}
