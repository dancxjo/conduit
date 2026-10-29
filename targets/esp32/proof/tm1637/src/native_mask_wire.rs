//! Authenticated finite cold context ingress. Nothing here releases a Step or
//! electrical effect. Request seal binds the complete received Plan bytes.
use crate::native_carrier::Carrier;
use crate::{
    native_control::write_all,
    native_owner_control as owner,
    native_owner_wire::{read_credited, read_exact},
    native_root::NativeRoot,
};
use alloc::vec::Vec;
use embassy_time::{with_timeout, Duration};
pub async fn receive(
    root: &mut NativeRoot,
    uart: &mut Carrier<'_>,
    buffer: &mut [u8],
) -> Result<(), ()> {
    with_timeout(Duration::from_secs(2), async {
        write_all(
            uart,
            b"\n{\"event\":\"native-mask-control-ready\",\"maximum_request_bytes\":2048}\n",
        )
        .await?;
        Ok::<(), ()>(())
    })
    .await
    .map_err(|_| ())??;
    let received = with_timeout(Duration::from_secs(5), async {
        let mut size = [0; 2];
        read_exact(uart, &mut size).await?;
        let n = usize::from(u16::from_le_bytes(size));
        if n == 0 || n > owner::MAXIMUM_REQUEST_BYTES || n > buffer.len() {
            return Err(());
        }
        read_credited(uart, &mut buffer[..n]).await?;
        let mut signature = [0; 64];
        read_credited(uart, &mut signature).await?;
        Ok((n, signature))
    })
    .await;
    let mut authenticated = false;
    let mut receipt = None;
    let mut refusal = Some("mask-authentication");
    if let Ok(Ok((n, signature))) = received {
        if let Ok(request) = serde_json_core::from_slice::<owner::Request<'_>>(&buffer[..n]) {
            if request.1 == n {
                if let Ok(token) = root.authenticate_owner(request.0, &signature) {
                    authenticated = true;
                    if token.operation() == owner::Operation::Retire {
                        let result = root.retire_mask_context(token);
                        refusal = result.as_ref().err().copied();
                        receipt = result.ok();
                    } else {
                        let wire=with_timeout(Duration::from_secs(10),async {
                        write_all(uart,b"\n{\"event\":\"native-mask-context-ready\",\"maximum_plan_bytes\":8192}\n").await?;
                        let mut size=[0;2];read_exact(uart,&mut size).await?;
                        let n=usize::from(u16::from_le_bytes(size));
                        if n==0 || n>8192 {return Err(());}
                        let mut wire=Vec::new();wire.try_reserve_exact(n).map_err(|_|())?;wire.resize(n,0);
                        read_credited(uart,&mut wire).await?;Ok(wire.into_boxed_slice())
                    }).await;
                        let result = match wire {
                            Ok(Ok(wire)) => root.stage_mask_context(token, wire),
                            _ => Err("mask-plan-ingress"),
                        };
                        refusal = result.as_ref().err().copied();
                        receipt = result.ok();
                    }
                }
            }
        }
    }
    #[derive(serde::Serialize)]
    struct Reply<'a> {
        event: &'static str,
        authenticated: bool,
        refusal: Option<&'a str>,
        acknowledgement: Option<&'a crate::native_owner_receipt::Receipt>,
        acknowledgement_signature: Option<&'a [u8]>,
        sequence: u64,
        physical_mask_plot_installed: bool,
        root_heap_used_bytes: usize,
        root_heap_peak_bytes: usize,
    }
    let signature = receipt.as_ref().map(|r| root.sign_owner_receipt(r));
    let stats = esp_alloc::HEAP.stats();
    let n = serde_json_core::to_slice(
        &Reply {
            event: "native-mask-control",
            authenticated,
            refusal,
            acknowledgement: receipt.as_ref(),
            acknowledgement_signature: signature.as_ref().map(|s| s.as_slice()),
            sequence: root.owner_control_sequence(),
            physical_mask_plot_installed: false,
            root_heap_used_bytes: stats.current_usage,
            root_heap_peak_bytes: stats.max_usage,
        },
        buffer,
    )
    .map_err(|_| ())?;
    with_timeout(Duration::from_secs(2), async {
        write_all(uart, b"\n").await?;
        write_all(uart, &buffer[..n]).await?;
        write_all(uart, b"\n").await
    })
    .await
    .map_err(|_| ())?
}
