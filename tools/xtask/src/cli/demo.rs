//! Repository demonstration arguments.
use clap::{Args, Subcommand, ValueEnum};

#[derive(Args, Debug)]
pub struct DemoArgs {
    #[command(subcommand)]
    pub command: DemoCommand,
}

#[derive(Subcommand, Debug)]
pub enum DemoCommand {
    /// Open the executable Conduit Tour through the browser Host.
    Tour,
    /// Birth a Body and arrive in its listening Forms through the browser Host.
    Workspace(crate::commands::workspace::WorkspaceArgs),
    /// Run the native Signal Form through the production kernel.
    Std,
    /// Run the three-sink Form entirely on the native Host.
    Triple,
    /// Build and launch the native Patchbay from this checkout.
    Patchbay(PatchbayDemoArgs),
    /// Let one Body BIRTH and open its canonical native Parts experience.
    BodyMembership,
    /// Open the authored physical-environment Patchbay demonstration.
    Environment,
    /// Rehearse a canonical Form against the authored environment before Wake.
    Prewake,
    /// Open the golden native Text Lab in effect-free PREWAKE, ready for the ordinary lifecycle.
    TextLab,
    /// Run the S4 distributed toggle proof interactively.
    Toggle,
    /// Run the attached C3-button/Pico synchronized physical light switch.
    LightSwitch(LightSwitchDemoArgs),
    /// Run the canonical button Form with scripted input and an acquired Pico LED.
    ButtonIndicator(crate::commands::button_indicator::ButtonIndicatorArgs),
    /// Run the Conduit-driven project homepage interactively.
    Site,
    /// Run the pinned Tongues text-to-speech starter through an ordinary Conduit Play.
    Tongues,
    /// Run the bounded real-data Tongues paired-latent research capstone.
    TonguesResearch,
    /// Analyze the frozen Tongues latent dynamics with bounded controls.
    TonguesAnalysis,
}

#[derive(Args, Debug)]
pub struct LightSwitchDemoArgs {
    /// Stable USB-UART path for the inspected ESP32-C3 DevKitM-1.
    #[arg(
        long,
        default_value = "/dev/serial/by-id/usb-Silicon_Labs_CP2102N_USB_to_UART_Bridge_Controller_dcf8355da19ded11a7205f84e259fb3e-if00-port0"
    )]
    pub c3_port: std::path::PathBuf,
    /// Stable CDC 0 command path for the light-switch Pico W image.
    #[arg(
        long,
        default_value = "/dev/serial/by-id/usb-Conduit_Pico_W_Light_Switch_conduit-pico-w-light-switch-if00"
    )]
    pub pico_link_port: std::path::PathBuf,
    /// Stable CDC 1 receipt path for the light-switch Pico W image.
    #[arg(
        long,
        default_value = "/dev/serial/by-id/usb-Conduit_Pico_W_Light_Switch_conduit-pico-w-light-switch-if02"
    )]
    pub pico_sign_port: std::path::PathBuf,
    /// Number of physical presses required before the bounded demo terminates.
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=16))]
    pub presses: u8,
}

#[derive(Args, Debug, Default)]
pub struct PatchbayDemoArgs {
    /// Select the Host realization used to manifest the shared entrance.
    #[arg(long, value_enum, default_value_t = PatchbayHost::Native)]
    pub on: PatchbayHost,
    /// Run the finite first-run authoring-to-Play acceptance journey.
    #[arg(long)]
    pub first_run_proof: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PatchbayHost {
    #[default]
    Native,
    Browser,
}
