//! Return the exact completed kernel, surface and frame custody after retirement.
use super::*;

impl BoundReadOnlyMask {
    pub(super) fn complete_frame(
        &mut self,
        storage: RetainedNativeMaskStorage,
        evidence: RetiredNativeMaskEvidence,
    ) -> Result<SelectedNativeMaskEvidence, String> {
        drop(self.scene.take());
        self.storage = Some(storage);
        // The completed local Show no longer owns an input route. Drop
        // its compositor identity heaps before transferring the frame
        // lease, while retaining/reusing the actual raster allocation.
        self.compositor
            .remove_surface(SURFACE)
            .and_then(|()| {
                self.compositor.admit_surface(
                    SURFACE,
                    conduit_face::LayoutRect {
                        x: 0,
                        y: 0,
                        width: self.width,
                        height: self.height,
                    },
                    0,
                )
            })
            .map_err(|error| {
                self.failed = true;
                format!("native Mask surface retirement: {error:?}")
            })?;
        let frame = self
            .frame
            .take()
            .ok_or("native Mask frame custody absent")?;
        let ActiveFrameCustody {
            composition,
            possession,
            _heap,
            pool,
        } = frame;
        let possession = possession.complete_and_retire().map_err(|error| {
            self.failed = true;
            format!("native Mask resource-Cord retirement refused: {error:?}")
        })?;
        let frame = FrameCustody {
            composition,
            possession,
            _heap,
            pool,
        };
        Ok(SelectedNativeMaskEvidence { evidence, frame })
    }
}
