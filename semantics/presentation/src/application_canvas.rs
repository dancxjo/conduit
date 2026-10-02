//! Finite graph content for the existing downstream application canvas.
//! Geometry is a Mask concern; these records retain exact semantic identities.
use crate::ApplicationViewRefusal;
use alloc::{collections::BTreeSet, format, string::String, vec::Vec};

pub const CANVAS_SCHEMA: &str = "conduit.presentation/graph-canvas@1";
pub const MAX_CANVAS_BYTES: usize = 65_536;
const MAX_FIELD_BYTES: usize = 1024;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasNode {
    pub identity: String,
    pub label: String,
    pub kind: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasPort {
    pub identity: String,
    pub owner: String,
    pub label: String,
    pub direction: String,
    pub info: String,
    pub temporal: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasCord {
    pub identity: String,
    pub source: String,
    pub sink: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationCanvas {
    pub checked_plot: String,
    pub expanded_plot: String,
    pub plan: String,
    pub body_plan: String,
    pub play: String,
    pub selected: String,
    pub nodes: Vec<CanvasNode>,
    pub ports: Vec<CanvasPort>,
    pub cords: Vec<CanvasCord>,
}
impl ApplicationCanvas {
    pub fn validate(&self) -> Result<(), ApplicationViewRefusal> {
        let invalid = ApplicationViewRefusal::InvalidControlValue;
        if self.nodes.len() > 128
            || self.ports.len() > 512
            || self.cords.len() > 512
            || [
                &self.checked_plot,
                &self.expanded_plot,
                &self.plan,
                &self.body_plan,
            ]
            .iter()
            .any(|s| s.is_empty())
        {
            return Err(invalid);
        }
        let mut subjects = BTreeSet::new();
        for node in &self.nodes {
            if node.identity.is_empty() || !subjects.insert(node.identity.as_str()) {
                return Err(invalid);
            }
        }
        for port in &self.ports {
            if port.identity.is_empty()
                || !subjects.insert(port.identity.as_str())
                || (!port.owner.is_empty() && !self.nodes.iter().any(|n| n.identity == port.owner))
                || !matches!(port.direction.as_str(), "input" | "output")
            {
                return Err(invalid);
            }
        }
        for cord in &self.cords {
            if cord.identity.is_empty()
                || !subjects.insert(cord.identity.as_str())
                || !self.ports.iter().any(|p| p.identity == cord.source)
                || !self.ports.iter().any(|p| p.identity == cord.sink)
            {
                return Err(invalid);
            }
        }
        if !self.selected.is_empty() && !subjects.contains(self.selected.as_str()) {
            return Err(invalid);
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<String, ApplicationViewRefusal> {
        self.validate()?;
        let mut out = String::new();
        for field in [
            CANVAS_SCHEMA,
            &self.checked_plot,
            &self.expanded_plot,
            &self.plan,
            &self.body_plan,
            &self.play,
            &self.selected,
        ] {
            push(&mut out, field)?;
        }
        push(&mut out, &format!("{}", self.nodes.len()))?;
        for node in &self.nodes {
            for field in [&node.identity, &node.label, &node.kind] {
                push(&mut out, field)?;
            }
        }
        push(&mut out, &format!("{}", self.ports.len()))?;
        for port in &self.ports {
            for field in [
                &port.identity,
                &port.owner,
                &port.label,
                &port.direction,
                &port.info,
                &port.temporal,
            ] {
                push(&mut out, field)?;
            }
        }
        push(&mut out, &format!("{}", self.cords.len()))?;
        for cord in &self.cords {
            for field in [&cord.identity, &cord.source, &cord.sink] {
                push(&mut out, field)?;
            }
        }
        Ok(out)
    }
    pub fn decode(source: &str) -> Result<Self, ApplicationViewRefusal> {
        if source.len() > MAX_CANVAS_BYTES {
            return Err(ApplicationViewRefusal::InvalidControlValue);
        }
        let mut cursor = Cursor(source);
        if cursor.field()? != CANVAS_SCHEMA {
            return Err(ApplicationViewRefusal::InvalidControlValue);
        }
        let mut result = Self {
            checked_plot: cursor.field()?,
            expanded_plot: cursor.field()?,
            plan: cursor.field()?,
            body_plan: cursor.field()?,
            play: cursor.field()?,
            selected: cursor.field()?,
            nodes: Vec::new(),
            ports: Vec::new(),
            cords: Vec::new(),
        };
        for _ in 0..cursor.count(128)? {
            result.nodes.push(CanvasNode {
                identity: cursor.field()?,
                label: cursor.field()?,
                kind: cursor.field()?,
            });
        }
        for _ in 0..cursor.count(512)? {
            result.ports.push(CanvasPort {
                identity: cursor.field()?,
                owner: cursor.field()?,
                label: cursor.field()?,
                direction: cursor.field()?,
                info: cursor.field()?,
                temporal: cursor.field()?,
            });
        }
        for _ in 0..cursor.count(512)? {
            result.cords.push(CanvasCord {
                identity: cursor.field()?,
                source: cursor.field()?,
                sink: cursor.field()?,
            });
        }
        if !cursor.0.is_empty() {
            return Err(ApplicationViewRefusal::InvalidControlValue);
        }
        result.validate()?;
        Ok(result)
    }
}
fn push(output: &mut String, field: &str) -> Result<(), ApplicationViewRefusal> {
    let prefix = format!("{}:", field.len());
    if field.len() > MAX_FIELD_BYTES || output.len() + prefix.len() + field.len() > MAX_CANVAS_BYTES
    {
        return Err(ApplicationViewRefusal::InvalidControlValue);
    }
    output.push_str(&prefix);
    output.push_str(field);
    Ok(())
}
struct Cursor<'a>(&'a str);
impl Cursor<'_> {
    fn field(&mut self) -> Result<String, ApplicationViewRefusal> {
        let error = || ApplicationViewRefusal::InvalidControlValue;
        let (size, rest) = self.0.split_once(':').ok_or_else(error)?;
        if size.len() > 4 || size.is_empty() || !size.bytes().all(|b| b.is_ascii_digit()) {
            return Err(error());
        }
        let size: usize = size.parse().map_err(|_| error())?;
        if size > MAX_FIELD_BYTES {
            return Err(error());
        }
        let value = rest.get(..size).ok_or_else(error)?;
        self.0 = rest.get(size..).ok_or_else(error)?;
        Ok(value.into())
    }
    fn count(&mut self, maximum: usize) -> Result<usize, ApplicationViewRefusal> {
        let value = self.field()?;
        let count = value
            .parse::<usize>()
            .map_err(|_| ApplicationViewRefusal::InvalidControlValue)?;
        if count > maximum {
            return Err(ApplicationViewRefusal::InvalidControlValue);
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn canvas() -> ApplicationCanvas {
        ApplicationCanvas {
            checked_plot: "checked".into(),
            expanded_plot: "expanded".into(),
            plan: "plan".into(),
            body_plan: "body-plan".into(),
            play: String::new(),
            selected: "gear".into(),
            nodes: alloc::vec![CanvasNode {
                identity: "gear".into(),
                label: "Café 🎵".into(),
                kind: "text/upper".into()
            }],
            ports: Vec::new(),
            cords: Vec::new(),
        }
    }
    #[test]
    fn unicode_round_trip_and_unknown_selection_refusal() {
        let mut graph = canvas();
        assert_eq!(
            ApplicationCanvas::decode(&graph.encode().unwrap()).unwrap(),
            graph
        );
        graph.selected = "absent".into();
        assert!(graph.encode().is_err());
    }
    #[test]
    fn bounds_and_dangling_endpoints_refuse() {
        let mut graph = canvas();
        graph.cords.push(CanvasCord {
            identity: "cord".into(),
            source: "absent".into(),
            sink: "absent".into(),
        });
        assert!(graph.encode().is_err());
        let mut graph = canvas();
        graph.nodes[0].label = "a".repeat(1025);
        assert!(graph.encode().is_err());
        let encoded = canvas().encode().unwrap();
        assert!(ApplicationCanvas::decode(&(encoded + "extra")).is_err());
        assert!(ApplicationCanvas::decode("9999:x").is_err());
    }
}
