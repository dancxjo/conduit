//! Physical three-participant, one-generation distributed Lenia proof.

use std::{str::FromStr, time::Duration};

use conduit_bluetooth::BleGattProfile;
use conduit_core::{
    LeniaLineFrameIdentity, LeniaLineFrameView, LeniaParameters, LeniaPartition,
    LeniaRegionChunkKind, LeniaRegionId, LeniaRegionResult, LeniaRegionTransferIdentity,
    LENIA_LINE_FRAME_MAX_BYTES, LENIA_REGION_CHUNK_MAX_BYTES, LENIA_REGION_CHUNK_MAX_CELLS,
};
use conduit_std_host::bluetooth_gatt::{discover_ble_gatt_candidate, BluezBleGattLine};

const IO_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 9 && args.len() != 11 {
        return Err("usage: distributed-lenia-probe <adapter> <wroom-address> <wroom-boot> <c3-address> <c3-boot> <pico-address> <pico-boot> <output.pgm> [--withhold-region 2]".into());
    }
    let withhold_pico = args.get(9).map(String::as_str) == Some("--withhold-region")
        && args.get(10).map(String::as_str) == Some("2");
    if args.len() == 11 && !withhold_pico {
        return Err("the bounded loss proof only permits --withhold-region 2".into());
    }
    let initial = conduit_core::orbium_seed(32, 32, 1).map_err(debug_error)?;
    let partition =
        LeniaPartition::vertical(&initial, &conduit_core::DISTRIBUTED_LENIA_REGION_WIDTHS)
            .map_err(debug_error)?;
    let direct = initial
        .evolve_reference(LeniaParameters::ORBIUM, 1)
        .map_err(debug_error)?;
    let work0 = partition
        .prepare_region(&initial, LeniaRegionId(0), LeniaParameters::ORBIUM)
        .map_err(debug_error)?;
    let work1 = partition
        .prepare_region(&initial, LeniaRegionId(1), LeniaParameters::ORBIUM)
        .map_err(debug_error)?;
    let work2 = partition
        .prepare_region(&initial, LeniaRegionId(2), LeniaParameters::ORBIUM)
        .map_err(debug_error)?;
    let adapter = args[1].clone();
    // BlueZ admits only one discovery procedure per controller, and this
    // controller cannot scan for a third peer while two connections are open.
    // Resolve all exact peers first, connect them serially, then run the six
    // admitted directional Lines concurrently.
    let wroom_address = discover(&adapter, &args[2], 0).await?;
    let c3_address = discover(&adapter, &args[4], 1).await?;
    let pico_address = if withhold_pico {
        None
    } else {
        Some(discover(&adapter, &args[6], 2).await?)
    };
    // The CYW43439 pairing exchange needs the controller before two other
    // connections consume its finite scheduling budget.
    let pico_line = if let Some(address) = pico_address {
        Some(connect(&adapter, address, 2).await?)
    } else {
        None
    };
    let wroom_line = connect(&adapter, wroom_address, 0).await?;
    let c3_line = connect(&adapter, c3_address, 1).await?;
    let wroom = participant(
        wroom_line,
        conduit_alife::DISTRIBUTED_LENIA_WROOM_HOST_ID,
        &args[3],
        work0,
        0,
    );
    let c3 = participant(
        c3_line,
        conduit_alife::DISTRIBUTED_LENIA_C3_HOST_ID,
        &args[5],
        work1,
        1,
    );
    if withhold_pico {
        let (r0, r1) = tokio::join!(wroom, c3);
        let results = [
            r0.map_err(|error| format!("region 0 failed: {error}"))?,
            r1.map_err(|error| format!("region 1 failed: {error}"))?,
        ];
        if partition.join(&results).is_ok() {
            return Err("withheld participant falsely completed the generation".into());
        }
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "success": true, "schema": "conduit.alife/distributed-lenia-loss-physical@1",
                "plan_id": conduit_alife::exact_distributed_lenia_plan()?.plan.plan_id.as_str(),
                "completed": false, "received_regions": [0, 1], "missing_region": 2,
                "terminal": "participant-withheld",
            }))?
        );
        return Ok(());
    }
    let pico = participant(
        pico_line.expect("non-withheld Pico Line must be connected"),
        conduit_alife::DISTRIBUTED_LENIA_PICO_HOST_ID,
        &args[7],
        work2,
        2,
    );
    let (r0, r1, r2) = tokio::join!(wroom, c3, pico);
    let joined = partition
        .join(&[
            r0.map_err(|error| format!("region 0 failed: {error}"))?,
            r1.map_err(|error| format!("region 1 failed: {error}"))?,
            r2.map_err(|error| format!("region 2 failed: {error}"))?,
        ])
        .map_err(debug_error)?;
    let joined_digest = joined.semantic_digest().map_err(debug_error)?;
    let direct_digest = direct.semantic_digest().map_err(debug_error)?;
    if joined_digest != direct_digest {
        return Err("joined field disagrees with direct oracle".into());
    }
    let bitmap = conduit_alife::lenia_field_to_gray8(&joined)
        .map_err(|_| "gray8 semantic lowering refused")?;
    let mut pgm = format!("P5\n{} {}\n255\n", bitmap.width(), bitmap.height()).into_bytes();
    pgm.extend_from_slice(bitmap.pixels());
    std::fs::write(&args[8], &pgm)?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "success": true, "schema": "conduit.alife/distributed-lenia-physical@1",
            "plan_id": conduit_alife::exact_distributed_lenia_plan()?.plan.plan_id.as_str(),
            "regions": [0, 1, 2], "direct_digest": hex(&direct_digest),
            "joined_digest": hex(&joined_digest), "bitmap_kind": "graphics/bitmap-gray8@1",
            "bitmap_width": bitmap.width(), "bitmap_height": bitmap.height(),
            "presenter": "image/x-portable-graymap", "presentation_path": args[8],
        }))?
    );
    Ok(())
}

async fn participant(
    mut line: BluezBleGattLine,
    host: &str,
    boot: &str,
    work: conduit_core::LeniaRegionWork,
    region: u8,
) -> Result<LeniaRegionResult, Box<dyn std::error::Error>> {
    let exact = conduit_alife::exact_distributed_lenia_plan()?;
    let bindings = conduit_alife::distributed_lenia_participant_bindings(&exact.plan, host, boot)?;
    let session_id = [region.wrapping_add(1); 16];
    let transfer = LeniaRegionTransferIdentity {
        kind: LeniaRegionChunkKind::Work,
        field_id: work.field_id,
        generation: work.generation,
        field_width: work.field_width,
        field_height: work.field_height,
        region: work.region,
        halo: work.halo,
        total_cells: work.expanded_cells().len() as u32,
    };
    let mut chunk = [0; LENIA_REGION_CHUNK_MAX_BYTES];
    let mut frame = [0; LENIA_LINE_FRAME_MAX_BYTES];
    let mut offset = 0;
    while offset < transfer.total_cells {
        let count = (transfer.total_cells - offset).min(LENIA_REGION_CHUNK_MAX_CELLS as u32) as u16;
        let start = offset as usize;
        let chunk_len = transfer
            .chunk(offset, count)
            .map_err(debug_error)?
            .encode(
                &work.expanded_cells()[start..start + usize::from(count)],
                &mut chunk,
            )
            .map_err(debug_error)?;
        let id = &bindings.work;
        let frame_len = LeniaLineFrameIdentity {
            plan_id: id.plan_id.as_str(),
            play_id: id.play_id.as_str(),
            line_id: id.line_id.as_str(),
            source_host_id: id.source_host_id.as_str(),
            source_boot_id: id.source_boot_id.as_str(),
            sink_host_id: id.sink_host_id.as_str(),
            sink_boot_id: boot,
            session_id,
        }
        .encode(&chunk[..chunk_len], &mut frame)
        .map_err(debug_error)?;
        tokio::time::timeout(IO_TIMEOUT, line.send_frame(&frame[..frame_len]))
            .await?
            .map_err(debug_error)?;
        offset += u32::from(count);
    }
    receive_result(&mut line, &bindings.result, boot, session_id, transfer).await
}

async fn discover(
    adapter: &str,
    address: &str,
    region: u8,
) -> Result<[u8; 6], Box<dyn std::error::Error>> {
    let address = bluer::Address::from_str(address)?.0;
    let candidate = discover_ble_gatt_candidate(adapter, address)
        .await
        .map_err(|error| format!("region {region} discover: {error:?}"))?;
    Ok(candidate.address)
}

async fn connect(
    adapter: &str,
    address: [u8; 6],
    region: u8,
) -> Result<BluezBleGattLine, Box<dyn std::error::Error>> {
    BluezBleGattLine::pair_and_connect(adapter, address, BleGattProfile::FIRST)
        .await
        .map_err(|error| format!("region {region} connect: {error:?}").into())
}

async fn receive_result(
    line: &mut BluezBleGattLine,
    binding: &conduit_alife::DistributedLeniaLineBinding,
    participant_boot: &str,
    session_id: [u8; 16],
    work: LeniaRegionTransferIdentity,
) -> Result<LeniaRegionResult, Box<dyn std::error::Error>> {
    let result_total = usize::from(work.region.width) * usize::from(work.field_height);
    let mut cells = vec![0; result_total];
    let mut admitted = 0usize;
    // The BlueZ Line API requires the full admitted transport frame bound even
    // though the Lenia envelope has a smaller semantic bound.
    let mut bytes = [0; conduit_alife::DISTRIBUTED_LENIA_FRAME_BYTES as usize];
    while admitted < result_total {
        let length = tokio::time::timeout(IO_TIMEOUT, line.receive_frame(&mut bytes))
            .await?
            .map_err(debug_error)?;
        let frame = LeniaLineFrameView::decode(&bytes[..length]).map_err(debug_error)?;
        let id = frame.identity;
        if id.plan_id != binding.plan_id.as_str()
            || id.play_id != binding.play_id.as_str()
            || id.line_id != binding.line_id.as_str()
            || id.source_host_id != binding.source_host_id.as_str()
            || id.source_boot_id != participant_boot
            || id.sink_host_id != binding.sink_host_id.as_str()
            || id.sink_boot_id != binding.sink_boot_id.as_str()
            || id.session_id != session_id
            || frame.chunk.header.kind != LeniaRegionChunkKind::Result
            || frame.chunk.header.cell_offset as usize != admitted
        {
            return Err("result Line identity or ordering mismatch".into());
        }
        for index in 0..frame.chunk.cell_count() {
            cells[admitted + index] = frame.chunk.cell(index).map_err(debug_error)?;
        }
        admitted += frame.chunk.cell_count();
    }
    LeniaRegionResult::from_cells(
        work.field_id,
        work.generation + 1,
        work.field_width,
        work.field_height,
        work.region,
        cells,
    )
    .map_err(debug_error)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn debug_error(error: impl core::fmt::Debug) -> Box<dyn std::error::Error> {
    format!("{error:?}").into()
}
