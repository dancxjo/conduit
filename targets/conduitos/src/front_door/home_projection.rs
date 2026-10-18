//! Home's local reading state is context on the owner's Face, never lifecycle authority.
use super::*;
use alloc::vec::Vec;

impl FrontDoor {
    pub(super) fn home_face_content(&self) -> (Vec<FaceProperty>, Vec<FaceText>) {
        let Some(home) = self.home_model() else {
            return (vec![], vec![]);
        };
        let subject = "conduitos/shell/home";
        let mut properties = vec![];
        let mut texts = vec![];
        let mut text = |value: String| {
            texts.push(FaceText {
                subject: subject.into(),
                text: value,
            });
        };
        match home.view() {
            HomeView::Launcher => {
                for destination in conduit_home_model::HomeDestination::ALL {
                    text(destination.label().into());
                }
                text(format!(
                    "Selected destination: {}",
                    home.selected_destination().label()
                ));
            }
            HomeView::Plots => {
                for (index, plot) in crate::native_workset::inventory().iter().enumerate() {
                    text(format!("Installed Plot: {}", plot.title()));
                    if index == home.selected_plot_index() {
                        text(format!("Selected Plot: {}", plot.title()));
                    }
                }
                properties.push(FaceProperty {
                    subject: subject.into(),
                    name: "selected-plot-index".into(),
                    value: FacePropertyValue::Count(home.selected_plot_index() as u64),
                });
            }
            HomeView::Prompt => {
                text(format!("conduit> {}", home.command()));
                text(home.output().into());
                properties.extend([
                    FaceProperty {
                        subject: subject.into(),
                        name: "command".into(),
                        value: FacePropertyValue::Text(home.command().into()),
                    },
                    FaceProperty {
                        subject: subject.into(),
                        name: "command-result".into(),
                        value: FacePropertyValue::Text(home.output().into()),
                    },
                ]);
            }
            HomeView::Body => {
                text("Body status uses the current Body, Host, Wake, Plan, Play and Line identities on this Face".into());
            }
        }
        (properties, texts)
    }
}
