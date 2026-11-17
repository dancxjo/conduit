use super::*;
use crate::preparation_backing::{Backing, sum};
impl Backing for CanonicalStructuredStartupValue {
    fn backing(&self) -> Option<usize> {
        sum([
            self.value_type.backing(),
            match &self.node {
                CanonicalStructuredStartupNode::Literal { canonical } => Some(canonical.capacity()),
                CanonicalStructuredStartupNode::Parameter(v) => v.backing(),
                CanonicalStructuredStartupNode::Collection(v) => v.backing(),
                CanonicalStructuredStartupNode::Record(v) => v.backing(),
                CanonicalStructuredStartupNode::Variant { tag, payload } => {
                    sum([tag.backing(), payload.backing()])
                }
            },
        ])
    }
}
impl Backing for CanonicalStructuredStartupField {
    fn backing(&self) -> Option<usize> {
        sum([self.name.backing(), self.value.backing()])
    }
}
