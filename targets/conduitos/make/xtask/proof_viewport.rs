//! Finite proof requests, never boot framebuffer authority.
use super::ConduitosError;
use clap::{Args, ValueEnum};
use serde_json::{json, Value};
// Match native progress Face custody and journey correspondence bounds.
const MAX_RECORD_BYTES: usize = 96 * 1024;
const MAX_RECORDS: usize = 256;
const MAX_SERIAL_BYTES: usize = 16 * 1024 * 1024;
use std::process::Command;
#[derive(Args, Debug)]
pub struct JourneyArgs {
    /// Request EDID dimensions; refuse unless capture and guest Mask match.
    /// Omission preserves the ordinary proof's existing invocation.
    #[arg(long, value_enum)]
    pub viewport: Option<Viewport>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Viewport {
    #[value(name = "1280x800")]
    Wide,
    #[value(name = "640x480")]
    Constrained,
}
impl Viewport {
    pub(super) fn dimensions(self) -> (u64, u64) {
        match self {
            Self::Wide => (1280, 800),
            Self::Constrained => (640, 480),
        }
    }
    pub(super) fn capture(self, frame: &Value) -> Result<(), ConduitosError> {
        let (w, h) = self.dimensions();
        if frame["width"] != w || frame["height"] != h {
            return Err(refusal(
                "requested viewport differs from actual QMP capture",
            ));
        }
        Ok(())
    }
    pub(super) fn guest(self, serial: &str, boot: &Value) -> Result<Value, ConduitosError> {
        if serial.len() > MAX_SERIAL_BYTES {
            return Err(refusal("viewport serial bound exceeded"));
        }
        let mut shows = Vec::new();
        let mut focuses = Vec::new();
        for line in serial.split_inclusive('\n').filter(|l| l.ends_with('\n')) {
            if let Some(text) = line.strip_prefix("CONDUIT_NATIVE_FACE ") {
                if shows.len() == MAX_RECORDS || text.len() > MAX_RECORD_BYTES {
                    return Err(refusal("viewport Face record bound exceeded"));
                }
                let row: Value = serde_json::from_str(text)
                    .map_err(|_| refusal("invalid viewport Face receipt"))?;
                shows.push(row["show"].clone());
            }
            if let Some(text) = line.strip_prefix("CONDUIT_CRECHE_FOCUS ") {
                if focuses.len() == MAX_RECORDS || text.len() > MAX_RECORD_BYTES {
                    return Err(refusal("viewport focus record bound exceeded"));
                }
                let focus: Value = serde_json::from_str(text)
                    .map_err(|_| refusal("invalid viewport focus receipt"))?;
                let (w, h) = self.dimensions();
                if ["face_id", "show_id"]
                    .iter()
                    .any(|key| focus[*key].as_str().is_none_or(str::is_empty))
                    || focus["face_revision"].as_u64().is_none()
                {
                    return Err(refusal("missing viewport focus identity"));
                }
                if focus["display_bounds"]["x"].as_u64() != Some(0)
                    || focus["display_bounds"]["y"].as_u64() != Some(0)
                    || focus["display_bounds"]["width"] != w
                    || focus["display_bounds"]["height"] != h
                {
                    return Err(refusal(
                        "requested viewport differs from actual guest Mask bounds",
                    ));
                }
                if !shows.iter().any(|show| {
                    show["host_id"] == boot["host_id"]
                        && show["boot_id"] == boot["boot_id"]
                        && show["face_id"] == focus["face_id"]
                        && show["face_revision"] == focus["face_revision"]
                        && show["show_id"] == focus["show_id"]
                        && show["lifecycle"] == "Available"
                }) {
                    return Err(refusal(
                        "viewport focus lacks prior acknowledged Show custody",
                    ));
                }
                focuses.push(focus);
            }
        }
        Ok(
            json!({"evidence":"guest Mask display bounds with prior acknowledged Show", "provider_format":"not serialized by ordinary product receipt", "focus_records":focuses}),
        )
    }
}
/// This is the sole VGA argument insertion used by the live journey builder.
pub(super) fn append_vga(command: &mut Command, viewport: Option<Viewport>) {
    match viewport {
        None => {
            command.args(["-vga", "std"]);
        }
        Some(viewport) => {
            let device = match viewport {
                Viewport::Wide => "VGA,edid=on,xres=1280,yres=800",
                Viewport::Constrained => "VGA,edid=on,xres=640,yres=480",
            };
            command.args(["-vga", "none", "-device", device]);
        }
    }
}
fn refusal(detail: &str) -> ConduitosError {
    ConduitosError::refusal("proof-viewport-mismatch", detail)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_is_finite_and_opt_in() {
        use clap::Parser;
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            args: JourneyArgs,
        }
        assert!(Cli::try_parse_from(["proof"])
            .unwrap()
            .args
            .viewport
            .is_none());
        assert_eq!(
            Cli::try_parse_from(["proof", "--viewport", "640x480"])
                .unwrap()
                .args
                .viewport,
            Some(Viewport::Constrained)
        );
        assert!(Cli::try_parse_from(["proof", "--viewport", "800x600"]).is_err());
    }
    #[test]
    fn modes_emit_one_vga_and_refuse_capture_mismatch() {
        let mut default = Command::new("qemu");
        append_vga(&mut default, None);
        assert_eq!(
            default
                .get_args()
                .map(|arg| arg.to_str().unwrap())
                .collect::<Vec<_>>(),
            ["-vga", "std"]
        );
        for mode in [Viewport::Wide, Viewport::Constrained] {
            let mut c = Command::new("qemu");
            append_vga(&mut c, Some(mode));
            let args: Vec<_> = c.get_args().map(|arg| arg.to_str().unwrap()).collect();
            let expected = match mode {
                Viewport::Wide => "VGA,edid=on,xres=1280,yres=800",
                Viewport::Constrained => "VGA,edid=on,xres=640,yres=480",
            };
            assert_eq!(args, ["-vga", "none", "-device", expected]);
            assert_eq!(args.iter().filter(|arg| arg.starts_with("VGA,")).count(), 1);
            let (w, h) = mode.dimensions();
            assert!(mode.capture(&json!({"width":w,"height":h})).is_ok());
            assert!(mode.capture(&json!({"width":w+1,"height":h})).is_err());
        }
    }
    #[test]
    fn mask_requires_extent_and_prior_show_custody() {
        let boot = json!({"host_id":"host","boot_id":"boot"});
        let show = json!({"host_id":"host","boot_id":"boot","face_id":"face","face_revision":1,"show_id":"show","lifecycle":"Available"});
        let focus = json!({"display_bounds":{"x":0,"y":0,"width":640,"height":480},"face_id":"face","face_revision":1,"show_id":"show"});
        let serial = |s: &Value| {
            format!(
                "CONDUIT_NATIVE_FACE {}\nCONDUIT_CRECHE_FOCUS {focus}\n",
                json!({"show":s})
            )
        };
        assert!(Viewport::Constrained.guest(&serial(&show), &boot).is_ok());
        for field in ["x", "y"] {
            let mut changed = focus.clone();
            changed["display_bounds"][field] = json!(1);
            let prefix = format!("CONDUIT_NATIVE_FACE {}\n", json!({"show":show}));
            assert!(Viewport::Constrained
                .guest(
                    &(prefix.clone() + &format!("CONDUIT_CRECHE_FOCUS {changed}\n")),
                    &boot
                )
                .is_err());
            changed["display_bounds"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(Viewport::Constrained
                .guest(
                    &(prefix + &format!("CONDUIT_CRECHE_FOCUS {changed}\n")),
                    &boot
                )
                .is_err());
        }
        let face_line = format!("CONDUIT_NATIVE_FACE {}\n", json!({"show":show}));
        let focus_line = format!("CONDUIT_CRECHE_FOCUS {focus}\n");
        assert!(Viewport::Constrained
            .guest(
                &(face_line.clone() + &focus_line.repeat(MAX_RECORDS)),
                &boot
            )
            .is_ok());
        assert!(Viewport::Constrained
            .guest(
                &(face_line.clone() + &focus_line.repeat(MAX_RECORDS + 1)),
                &boot
            )
            .is_err());
        assert!(Viewport::Constrained
            .guest(&(face_line.repeat(MAX_RECORDS + 1) + &focus_line), &boot)
            .is_err());
        for prefix in ["CONDUIT_NATIVE_FACE ", "CONDUIT_CRECHE_FOCUS "] {
            assert!(Viewport::Constrained
                .guest(
                    &(prefix.to_owned() + &" ".repeat(MAX_RECORD_BYTES) + "{}\n"),
                    &boot
                )
                .is_err());
            assert!(Viewport::Constrained
                .guest(&(prefix.to_owned() + "malformed\n"), &boot)
                .is_err());
            // A live snapshot may omit only its final uncommitted line.
            assert!(Viewport::Constrained
                .guest(&(serial(&show) + prefix + "{incomplete"), &boot)
                .is_ok());
        }

        assert!(Viewport::Wide.guest(&serial(&show), &boot).is_err());
        for field in ["host_id", "boot_id", "face_id", "show_id"] {
            let mut wrong = show.clone();
            wrong[field] = json!("foreign");
            assert!(Viewport::Constrained.guest(&serial(&wrong), &boot).is_err());
        }
        assert!(Viewport::Constrained
            .guest(&format!("CONDUIT_CRECHE_FOCUS {focus}\n"), &boot)
            .is_err());
    }
}
