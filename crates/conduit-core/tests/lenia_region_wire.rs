use conduit_core::{
    LeniaFieldId, LeniaRegion, LeniaRegionChunkAdmission, LeniaRegionChunkAssembler,
    LeniaRegionChunkHeader, LeniaRegionChunkKind, LeniaRegionChunkRefusal, LeniaRegionChunkView,
    LeniaRegionId, LENIA_REGION_CHUNK_MAX_BYTES, LENIA_REGION_CHUNK_MAX_CELLS,
};

fn header(kind: LeniaRegionChunkKind, offset: u32, count: u16) -> LeniaRegionChunkHeader {
    LeniaRegionChunkHeader {
        kind,
        field_id: LeniaFieldId([3; 16]),
        generation: if kind == LeniaRegionChunkKind::Work {
            4
        } else {
            5
        },
        field_width: 128,
        field_height: 128,
        region: LeniaRegion {
            id: LeniaRegionId(1),
            x: 40,
            width: 43,
        },
        halo: if kind == LeniaRegionChunkKind::Work {
            13
        } else {
            0
        },
        total_cells: if kind == LeniaRegionChunkKind::Work {
            69 * 154
        } else {
            43 * 128
        },
        cell_offset: offset,
        cell_count: count,
    }
}

#[test]
fn assembler_requires_one_exact_ordered_transfer_before_exposing_work() {
    let first_header = header(LeniaRegionChunkKind::Work, 0, 2);
    let mut storage = vec![0_u32; first_header.total_cells as usize];
    let mut assembler =
        LeniaRegionChunkAssembler::new(first_header.transfer_identity(), &mut storage).unwrap();
    let mut encoded = [0_u8; LENIA_REGION_CHUNK_MAX_BYTES];
    let first_length = first_header.encode(&[10, 20], &mut encoded).unwrap();
    assert_eq!(
        assembler.admit(LeniaRegionChunkView::decode(&encoded[..first_length]).unwrap()),
        Ok(LeniaRegionChunkAdmission::Progress { admitted_cells: 2 })
    );
    assert_eq!(
        assembler.admit(LeniaRegionChunkView::decode(&encoded[..first_length]).unwrap()),
        Err(LeniaRegionChunkRefusal::UnexpectedOffset)
    );
    assert_eq!(
        assembler.work_view().unwrap_err(),
        LeniaRegionChunkRefusal::Incomplete
    );

    let mut offset = 2;
    while offset < first_header.total_cells {
        let count =
            (first_header.total_cells - offset).min(LENIA_REGION_CHUNK_MAX_CELLS as u32) as u16;
        let cells = vec![7; usize::from(count)];
        let chunk = first_header
            .transfer_identity()
            .chunk(offset, count)
            .unwrap();
        let length = chunk.encode(&cells, &mut encoded).unwrap();
        let disposition = assembler
            .admit(LeniaRegionChunkView::decode(&encoded[..length]).unwrap())
            .unwrap();
        offset += u32::from(count);
        assert_eq!(
            disposition,
            if offset == first_header.total_cells {
                LeniaRegionChunkAdmission::Complete
            } else {
                LeniaRegionChunkAdmission::Progress {
                    admitted_cells: offset,
                }
            }
        );
    }
    let work = assembler.work_view().unwrap();
    assert_eq!(
        work.expanded_cells().len(),
        first_header.total_cells as usize
    );
}

#[test]
fn exact_maximum_chunk_round_trips_without_allocation() {
    let cells = [12_345; LENIA_REGION_CHUNK_MAX_CELLS];
    let mut encoded = [0_u8; LENIA_REGION_CHUNK_MAX_BYTES];
    let length = header(
        LeniaRegionChunkKind::Work,
        0,
        LENIA_REGION_CHUNK_MAX_CELLS as u16,
    )
    .encode(&cells, &mut encoded)
    .unwrap();
    assert_eq!(length, 1_024);
    let decoded = LeniaRegionChunkView::decode(&encoded).unwrap();
    assert_eq!(decoded.header, header(LeniaRegionChunkKind::Work, 0, 243));
    assert_eq!(decoded.cell(0).unwrap(), 12_345);
    assert_eq!(decoded.cell(242).unwrap(), 12_345);
}

#[test]
fn malformed_identity_range_and_cell_are_distinct_refusals() {
    let cells = [1_u32];
    let mut encoded = [0_u8; LENIA_REGION_CHUNK_MAX_BYTES];
    let length = header(LeniaRegionChunkKind::Result, 0, 1)
        .encode(&cells, &mut encoded)
        .unwrap();

    let mut wrong_magic = encoded;
    wrong_magic[0] ^= 1;
    assert_eq!(
        LeniaRegionChunkView::decode(&wrong_magic[..length]).unwrap_err(),
        LeniaRegionChunkRefusal::WrongMagic
    );
    let invalid_range = LeniaRegionChunkHeader {
        cell_offset: 43 * 128,
        ..header(LeniaRegionChunkKind::Result, 0, 1)
    };
    assert_eq!(
        invalid_range.encode(&cells, &mut encoded),
        Err(LeniaRegionChunkRefusal::InvalidCellRange)
    );
    assert_eq!(
        header(LeniaRegionChunkKind::Result, 0, 1)
            .encode(&[conduit_core::LENIA_Q16_ONE + 1], &mut encoded),
        Err(LeniaRegionChunkRefusal::CellOutOfRange)
    );
}
