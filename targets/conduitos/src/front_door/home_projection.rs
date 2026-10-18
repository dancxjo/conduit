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
        properties.push(FaceProperty {
            subject: subject.into(),
            name: "installed-plot-count".into(),
            value: FacePropertyValue::Count(self.home_installed_plots().len() as u64),
        });
        for (index, plot) in self.home_installed_plots().iter().enumerate() {
            properties.extend([
                FaceProperty {
                    subject: subject.into(),
                    name: format!("installed-plot/{index}/title"),
                    value: FacePropertyValue::Text(plot.title.into()),
                },
                FaceProperty {
                    subject: subject.into(),
                    name: format!("installed-plot/{index}/source-document-id"),
                    value: FacePropertyValue::Identity(
                        plot.plot.source_document_id.as_str().into(),
                    ),
                },
                FaceProperty {
                    subject: subject.into(),
                    name: format!("installed-plot/{index}/checked-plot-id"),
                    value: FacePropertyValue::Identity(plot.plot.checked_plot_id.as_str().into()),
                },
            ]);
        }
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
                for (index, plot) in self.home_installed_plots().iter().enumerate() {
                    text(format!("Installed Plot: {}", plot.title));
                    if index == home.selected_plot_index() {
                        text(format!("Selected Plot: {}", plot.title));
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
        if let Some(reason) = self.home_refusal() {
            texts.push(FaceText {
                subject: subject.into(),
                text: reason.into(),
            });
            properties.push(FaceProperty {
                subject: subject.into(),
                name: "unavailable-request".into(),
                value: FacePropertyValue::Text(reason.into()),
            });
        }
        (properties, texts)
    }
}
