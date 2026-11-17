use super::HomogeneousVariadicFore;
use crate::preparation_backing::{sum, Backing};
impl Backing for HomogeneousVariadicFore {
    fn backing(&self) -> Option<usize> {
        sum([self.prototype.backing(), self.output.backing()])
    }
}
