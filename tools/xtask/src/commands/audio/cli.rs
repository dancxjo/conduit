use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct AudioArgs {
    #[command(subcommand)]
    pub command: AudioCommand,
}

#[derive(Subcommand, Debug)]
pub enum AudioCommand {
    /// List freshly observed ALSA playback resources without opening them.
    List,
    /// Render the original startup cue to a new WAV file without opening audio.
    RenderStartupCue {
        #[arg(long)]
        output: PathBuf,
    },
    /// Run the bounded audible specimen through one exact selected output.
    PlaybackProof {
        /// Exact ALSA card identity from `cargo xtask audio list`.
        #[arg(long)]
        card_id: String,
        /// Exact ALSA device number on the selected card.
        #[arg(long)]
        device: u16,
        /// Explicitly authorize sounding the selected output for this proof.
        #[arg(long, required = true)]
        authorize_output: bool,
    },
}
