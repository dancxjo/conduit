use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct StartupCueArgs {
    /// New WAV file that receives the bounded startup cue.
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Args, Debug)]
pub struct AudioPlaybackArgs {
    /// Exact ALSA card identity from `cargo xtask doctor audio`.
    #[arg(long)]
    pub card_id: String,
    /// Exact ALSA device number on the selected card.
    #[arg(long)]
    pub device: u16,
    /// Explicitly authorize sounding the selected output for this proof.
    #[arg(long, required = true)]
    pub authorize_output: bool,
}
