//! Exact request correlation and graphical admission for general entrance queries.
use super::*;
use alloc::string::String;
use conduitos_mask_core::{
    control::Operation,
    entrance::{self, Detail, Facts},
};
impl Owner {
    fn entrance_facts(&mut self, facts: &mut Facts) -> Result<[u8; 32], Refusal> {
        self.control(
            &facts.scope.control.host,
            &facts.scope.control.boot,
            Operation::Read,
        )?;
        facts.scope.control.session = self.service()?.instance();
        facts.validate()?;
        facts.digest()
    }
    pub(crate) fn entrance_scene(
        &mut self,
        mut facts: Facts,
    ) -> Result<(bool, GraphicsScene), Refusal> {
        let result = (|| {
            let digest = self.entrance_facts(&mut facts)?;
            let scope = facts.scope.clone();
            let workspace_allowed = facts.lifecycle[1].is_some() && facts.connectivity.is_none();
            let command = Command::Entrance {
                expected: self.control_revision,
                facts,
            };
            let bytes = self.service()?.command(&command)?;
            let (receipt, scene) = entrance::decode_scene(&bytes)?;
            if receipt.scope != scope
                || receipt.control_revision != self.control_revision
                || receipt.digest != digest
            {
                return Err(Refusal::Malformed);
            }
            if receipt.workspace {
                if !workspace_allowed || !scene.commands().is_empty() {
                    return Err(Refusal::Malformed);
                }
            } else {
                let viewport = LayoutRect {
                    x: 0,
                    y: 0,
                    width: scope.width,
                    height: scope.height,
                };
                // Canonical general text deliberately clips to its own bounded
                // pane, while the background clips to the admitted viewport.
                // Preserve that established behavior without accepting arbitrary clips.
                if scene.commands().is_empty()
                    || scene.commands().iter().any(|c| {
                        c.clip != viewport
                            && (c.clip != c.bounds
                                || c.bounds.width != 610
                                || c.bounds.height != 18
                                || c.bounds.x < 0
                                || c.bounds.y < 0
                                || c.bounds.x > 26
                                || c.bounds.y > 206)
                    })
                {
                    return Err(Refusal::Malformed);
                }
            }
            Ok((receipt.workspace, scene))
        })();
        if result.is_err() {
            self.terminal();
        }
        result
    }
    pub(crate) fn entrance_detail(
        &mut self,
        mut facts: Facts,
    ) -> Result<(Detail, String), Refusal> {
        let result = (|| {
            let digest = self.entrance_facts(&mut facts)?;
            let scope = facts.scope.clone();
            let command = Command::Detail {
                expected: self.control_revision,
                facts,
            };
            let bytes = self.service()?.command(&command)?;
            let receipt: entrance::DetailReceipt = data::decode(&bytes)?;
            if receipt.scope != scope
                || receipt.control_revision != self.control_revision
                || receipt.digest != digest
                || receipt.value.len() > 2048
            {
                return Err(Refusal::Malformed);
            }
            Ok((receipt.kind, receipt.value))
        })();
        if result.is_err() {
            self.terminal();
        }
        result
    }
}
