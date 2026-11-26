//! Borrowed, allocation-free acknowledgement of a prepared Full Show.
use super::*;
pub(super) struct Ack<'a> {
    pub show: &'a str,
    pub face: &'a str,
    pub revision: &'a str,
}
pub(super) fn parse(bytes: &[u8]) -> Result<Ack<'_>, &'static str> {
    if bytes.len() > 2 * 256 + 20 + 96 {
        return Err("full acknowledgement extent");
    }
    let value = std::str::from_utf8(bytes).map_err(|_| "full acknowledgement UTF-8")?;
    if value.contains('\\') {
        return Err("full acknowledgement escaping unsupported");
    }
    let object = value
        .trim()
        .strip_prefix('{')
        .and_then(|v| v.strip_suffix('}'))
        .ok_or("full acknowledgement object")?;
    let (mut show, mut face, mut revision) = (None, None, None);
    for field in object.split(',') {
        let (key, value) = field.split_once(':').ok_or("full acknowledgement field")?;
        let key = quoted(key.trim())?;
        let value = quoted(value.trim())?;
        let slot = match key {
            "show_id" => &mut show,
            "face_id" => &mut face,
            "face_revision" => &mut revision,
            _ => return Err("full acknowledgement key"),
        };
        if slot.replace(value).is_some() {
            return Err("full acknowledgement duplicate");
        }
    }
    let ack = Ack {
        show: show.ok_or("full acknowledgement Show")?,
        face: face.ok_or("full acknowledgement Face")?,
        revision: revision.ok_or("full acknowledgement revision")?,
    };
    if ack.show.len() > 256
        || ack.face.len() > 256
        || ack.revision.len() > 20
        || !ack.revision.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("full acknowledgement identity extent");
    }
    Ok(ack)
}
fn quoted(value: &str) -> Result<&str, &'static str> {
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .ok_or("full acknowledgement string")?;
    if value.contains('"') || value.bytes().any(|b| b < 0x20) {
        return Err("full acknowledgement string spelling");
    }
    Ok(value)
}
impl OwnerBrowserMask {
    pub(super) fn acknowledge_full(&mut self, ack: Ack<'_>) -> Result<(), &'static str> {
        let full = self.full.as_mut().ok_or("Full Mask absent")?;
        if ack.show != self.show.show_id.as_str()
            || ack.face != self.face.identity.as_str()
            || ack.revision != full.revision
            || self.show.show.lifecycle != ManifestationLifecycle::Prepared
        {
            return Err("Full Show stale acknowledgement");
        }
        let output = self
            .scheduler
            .store_host_value(&full.manifestation)
            .map_err(|_| "Full Show storage pressure")?;
        self.scheduler
            .complete_host_call(
                self.pending_node,
                self.pending_request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(output, MASK_BYTES)
                            .map_err(|_| "Full Show value extent")?,
                    ),
                    failure: None,
                },
            )
            .map_err(|_| "Full Show completion refused")?;
        let offer = loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(self.show_boundary.endpoint, self.show_boundary.cord)
                .map_err(|_| "Full Show egress refused")?
            {
                break offer;
            }
            match self
                .scheduler
                .step()
                .map_err(|_| "Full Show kernel refused")?
            {
                SchedulerStatus::Progress { .. } => {}
                _ => return Err("Full Show missing egress"),
            }
        };
        if self
            .scheduler
            .host_value(offer.value)
            .map_err(|_| "Full Show bytes unavailable")?
            != full.manifestation
        {
            return Err("Full Show egress mismatch");
        }
        self.scheduler
            .remote_egress_accept(
                self.show_boundary.endpoint,
                self.show_boundary.cord,
                offer.sequence,
            )
            .map_err(|_| "Full Show accept refused")?;
        self.scheduler
            .remote_egress_delivered(
                self.show_boundary.endpoint,
                self.show_boundary.cord,
                offer.sequence,
            )
            .map_err(|_| "Full Show delivery refused")?;
        let pending = self
            .scheduler
            .next_host_request()
            .ok_or("Full Show missing interaction boundary")?;
        if pending.request != conduit_kernel::RequestId(1) {
            return Err("Full Show unexpected request");
        }
        self.pending_interaction = Some((pending.node, pending.request));
        self.show = full
            .available_show
            .take()
            .ok_or("Full Show already acknowledged")?;
        // The Available Show was validated against this immutable Face before
        // Play. No new projection, transition clone or serde buffer is built.
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn borrowed_ack_accepts_field_order_and_refuses_ambiguous_spelling() {
        let ack = parse(
            br#"{"face_revision":"18446744073709551615","face_id":"face/a","show_id":"show/a"}"#,
        )
        .unwrap();
        assert_eq!(ack.revision, "18446744073709551615");
        assert!(
            parse(br#"{"show_id":"a","face_id":"b","face_revision":"1","show_id":"c"}"#).is_err()
        );
        assert!(parse(br#"{"show_id":"a","face_id":"b","face_revision":"\u0031"}"#).is_err());
    }
}
