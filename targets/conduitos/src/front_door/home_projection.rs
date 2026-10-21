//! The picker observes only the exact residents in the current Body Play.
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use super::*;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use alloc::vec::Vec;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
impl FrontDoor {
    pub(super) fn home_face_content(&self) -> (Vec<FaceProperty>, Vec<FaceText>) {
        let Some(home) = self.home_model() else {
            return (vec![], vec![]);
        };
        let subject = "conduitos/shell/home";
        let plots = self.home_active_plots();
        let mut properties = vec![FaceProperty {
            subject: subject.into(),
            name: "active-plot-count".into(),
            value: FacePropertyValue::Count(plots.len() as u64),
        }];
        let mut texts = vec![];
        for (index, plot) in plots.iter().enumerate() {
            properties.extend([
                FaceProperty {
                    subject: subject.into(),
                    name: format!("active-plot/{index}/title"),
                    value: FacePropertyValue::Text(plot.title.into()),
                },
                FaceProperty {
                    subject: subject.into(),
                    name: format!("active-plot/{index}/source-document-id"),
                    value: FacePropertyValue::Identity(
                        plot.plot.source_document_id.as_str().into(),
                    ),
                },
                FaceProperty {
                    subject: subject.into(),
                    name: format!("active-plot/{index}/checked-plot-id"),
                    value: FacePropertyValue::Identity(plot.plot.checked_plot_id.as_str().into()),
                },
            ]);
            texts.push(FaceText {
                subject: subject.into(),
                text: format!("Active Plot: {}", plot.title),
            });
            if index == home.selected_plot_index() {
                texts.push(FaceText {
                    subject: subject.into(),
                    text: format!("Selected Plot: {}", plot.title),
                });
            }
        }
        (properties, texts)
    }
}
