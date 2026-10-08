//! One owner Face on the admitted native carrier, with an optional sealed return.

use super::*;

pub(super) fn serve_optional_face_snapshot(
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
    listener: &SecureWebSocketListener,
    state_dir: &Path,
    receipt: &PortableAdmissionReceipt,
) -> Result<Option<return_route::Grant>, String> {
    use std::io::ErrorKind;
    line.set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|error| format!("set owner Face deadline: {error:?}"))?;
    let mut bytes = vec![0; MAX_OWNER_FACE_RESPONSE_BYTES];
    let length = match line.receive_binary(&mut bytes) {
        Ok(length) => length,
        Err(SecureWebSocketError::Disconnected)
        | Err(SecureWebSocketError::Transport(
            ErrorKind::TimedOut
            | ErrorKind::WouldBlock
            | ErrorKind::UnexpectedEof
            | ErrorKind::ConnectionReset,
        )) => {
            return Ok(None);
        }
        Err(error) => return Err(format!("receive owner Face request: {error:?}")),
    };
    let (request, return_requested) = return_route::decode_face_request(&bytes[..length])?;
    let credential = &receipt.credential;
    if !request.has_exact_basis()
        || request.credential_id != credential.credential_id.as_str()
        || request.body_id != credential.body_id
        || request.part_id != credential.part_id
        || request.host_id != credential.host_id
        || request.boot_id != credential.boot_id
    {
        send(
            line,
            &OwnerFaceSnapshotResponse::Refused {
                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                code: "face-credential-mismatch".into(),
            },
        )?;
        return Err("owner Face request differs from the admitted guest".into());
    }
    let mut response = crate::durable_host_control::face_snapshot(state_dir, request)
        .unwrap_or_else(|error| OwnerFaceSnapshotResponse::Refused {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            code: if error == "face-frame-pressure" {
                "face-frame-pressure"
            } else {
                "face-unavailable"
            }
            .into(),
        });
    let mut grant = if return_requested
        && matches!(&response, OwnerFaceSnapshotResponse::Snapshot { presentation, .. }
            if crate::durable_host_control::has_native_return_route_intent(presentation))
    {
        Some(return_route::Grant::issue(receipt)?)
    } else {
        None
    };
    let selected = if let (Some(grant), OwnerFaceSnapshotResponse::Snapshot { route, .. }) =
        (grant.as_ref(), &mut response)
    {
        let result = crate::durable_host_control::local_face_snapshot(state_dir).and_then(
            |(_, owner_offer)| {
                let binding =
                    native_lines::binding_reference(listener, &grant.binding_reference(0));
                let (face_line, return_line) =
                    native_lines::offer_pair(&owner_offer, receipt, &binding);
                crate::durable_host_control::select_native_mask_route(
                    state_dir,
                    receipt.clone(),
                    face_line,
                    return_line,
                    grant.expires_at_millis(),
                )
            },
        );
        match result {
            Ok(selected) => {
                *route = Some(Box::new(selected));
                true
            }
            Err(error) => {
                eprintln!("native owner Mask route refused: {error}");
                false
            }
        }
    } else {
        false
    };
    if let OwnerFaceSnapshotResponse::Snapshot {
        interactions_admitted,
        ..
    } = &mut response
    {
        *interactions_admitted = selected;
    }
    if !selected {
        grant = None;
    }
    response_document::send(line, &response)?;
    if let Some(grant) = &grant {
        send(line, grant)?;
    }
    Ok(grant)
}
