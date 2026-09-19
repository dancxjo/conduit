use super::*;

#[test]
fn one_descriptor_ring_publishes_and_consumes_exactly() {
    let mut memory = QueueMemory([0; QUEUE_BYTES]);
    let mut queue = LegacyQueue::new(memory.0.as_mut_ptr());
    queue.set_descriptor_address(0, 0x1234_5000);
    queue.publish(0, 1514, true).unwrap();
    unsafe {
        assert_eq!(
            ptr::read_volatile(memory.0.as_ptr().cast::<u64>()),
            0x1234_5000
        );
        assert_eq!(
            ptr::read_volatile(memory.0.as_ptr().add(8).cast::<u32>()),
            1514
        );
        assert_eq!(
            ptr::read_volatile(memory.0.as_ptr().add(12).cast::<u16>()),
            DESCRIPTOR_WRITE
        );
        assert_eq!(
            ptr::read_volatile(memory.0.as_ptr().add(AVAILABLE_OFFSET + 2).cast::<u16>()),
            1
        );
        ptr::write_volatile(memory.0.as_mut_ptr().add(USED_OFFSET + 4).cast::<u32>(), 0);
        ptr::write_volatile(
            memory.0.as_mut_ptr().add(USED_OFFSET + 8).cast::<u32>(),
            1524,
        );
        ptr::write_volatile(memory.0.as_mut_ptr().add(USED_OFFSET + 2).cast::<u16>(), 1);
    }
    assert_eq!(queue.take_completion(), Ok(Some(1524)));
    assert_eq!(queue.take_completion(), Ok(None));
}

#[test]
fn queue_pressure_timeout_and_malformed_completion_are_distinct() {
    let mut memory = QueueMemory([0; QUEUE_BYTES]);
    let mut queue = LegacyQueue::new(memory.0.as_mut_ptr());
    queue.publish(0, 64, false).unwrap();
    assert_eq!(queue.publish(0, 64, false), Err(VirtioNetError::Pressure));
    assert_eq!(queue.wait_for_completion(1), Err(VirtioNetError::Timeout));
    unsafe {
        ptr::write_volatile(memory.0.as_mut_ptr().add(USED_OFFSET + 4).cast::<u32>(), 3);
        ptr::write_volatile(memory.0.as_mut_ptr().add(USED_OFFSET + 8).cast::<u32>(), 64);
        ptr::write_volatile(memory.0.as_mut_ptr().add(USED_OFFSET + 2).cast::<u16>(), 1);
    }
    assert_eq!(
        queue.take_completion(),
        Err(VirtioNetError::MalformedCompletion)
    );
}

#[test]
fn physical_regions_require_page_alignment_and_contiguity() {
    fn good(address: u64) -> Option<u64> {
        Some(address - 0xffff_ffff_8000_0000 + 0x20_0000)
    }
    fn gapped(address: u64) -> Option<u64> {
        let base = good(address)?;
        Some(if address & 0x1000 == 0 {
            base
        } else {
            base + 4096
        })
    }
    assert_eq!(
        physical_region(0xffff_ffff_8000_0000, 8192, good),
        Ok(0x20_0000)
    );
    assert_eq!(
        physical_region(0xffff_ffff_8000_0000, 8192, gapped),
        Err(VirtioNetError::DmaNotContiguous)
    );
    assert_eq!(
        physical_region(0xffff_ffff_8000_0001, 1, good),
        Err(VirtioNetError::DmaAddressInvalid)
    );
}
