//! Home's local reading state is context on the owner's Face, never lifecycle authority.
use super::{Controller, presentation::Request};
use alloc::vec::Vec;
use alloc::{format, string::String, vec};
use conduit_face::{FaceProperty, FacePropertyValue, FaceText};
use conduit_home_model::HomeView;

impl Controller {
    pub(crate) fn face_content(&self, input: &Request) -> (Vec<FaceProperty>, Vec<FaceText>) {
        let home = &self.model;
        let subject = "conduitos/shell/home";
        let mut properties = vec![];
        let mut texts = vec![];
        properties.push(FaceProperty {
            subject: subject.into(),
            name: "installed-plot-count".into(),
            value: FacePropertyValue::Count(input.plots.len() as u64),
        });
        for (index, plot) in input.plots.iter().enumerate() {
            properties.extend([
                FaceProperty {
                    subject: subject.into(),
                    name: format!("installed-plot/{index}/title"),
                    value: FacePropertyValue::Text(plot.title.clone()),
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
                for (index, plot) in input.plots.iter().enumerate() {
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
                for (name, value) in [
                    ("command", home.command()),
                    ("command-result", home.output()),
                ] {
                    properties.push(FaceProperty {
                        subject: subject.into(),
                        name: format!("{name}-empty"),
                        value: FacePropertyValue::Flag(value.is_empty()),
                    });
                    if !value.is_empty() {
                        properties.push(FaceProperty {
                            subject: subject.into(),
                            name: name.into(),
                            value: FacePropertyValue::Text(value.into()),
                        });
                    }
                }
                if !home.output().is_empty() {
                    text(home.output().into());
                }
            }
            HomeView::Body => {
                text("Body status uses the current Body, Host, Wake, Plan, Play and Line identities on this Face".into());
            }
        }
        if let Some(reason) = self.refusal.as_deref() {
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
