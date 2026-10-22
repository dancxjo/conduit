//! Source facts belong to the shared Face, independently of Mask geometry.
use super::*;
use alloc::string::ToString;

pub(super) fn append(content: &mut GraphFacts<'_>, subject: &str, cord: &PatchbayCord) {
    let Some(origin) = &cord.source_origin else {
        identity(content, subject, "source-origin", "unproven");
        return;
    };
    for (name, value) in [
        ("source-origin", origin.origin.as_str()),
        ("source-document-id", origin.source_document_id.as_str()),
        ("checked-source-plot-id", origin.checked_plot_id.as_str()),
        ("source-plot", origin.source_plot.as_str()),
    ] {
        identity(content, subject, name, value);
    }
    let mut path = String::new();
    for segment in &origin.plot_path {
        path.push_str(&segment.len().to_string());
        path.push(':');
        path.push_str(segment);
    }
    content.properties.push(FaceProperty {
        subject: subject.into(),
        name: "source-instance-path".into(),
        value: FacePropertyValue::Text(path),
    });
    let span = origin.source_span;
    for (name, value) in [
        ("source-byte-start", span.start),
        ("source-byte-end", span.end),
        ("source-line", span.line),
        ("source-column", span.column),
        ("source-end-line", span.end_line),
        ("source-end-column", span.end_column),
    ] {
        content.properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value: FacePropertyValue::Count(value as u64),
        });
    }
}

fn identity(content: &mut GraphFacts<'_>, subject: &str, name: &str, value: &str) {
    content.properties.push(FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value: FacePropertyValue::Identity(value.into()),
    });
}
