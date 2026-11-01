use super::*;
use view::{ReceiptCredentialRef, ReceiptIdentityRef};
#[derive(Debug, Clone)]
struct Row {
    admission: [u8; 32],
    credential: [u8; 32],
    texts: [u16; 4],
    issued: u64,
}
#[derive(Debug, Clone, Copy)]
struct Span {
    start: u32,
    length: u16,
}
#[derive(Debug, Clone)]
pub(super) struct Compact {
    pub(super) hot: Vec<AdmissionReceipt>,
    rows: Vec<Row>,
    pub(super) order: Vec<u8>,
    text: Vec<u8>,
    spans: Vec<Span>,
}
impl Compact {
    fn text(&self, index: u16) -> &str {
        let span = self.spans[usize::from(index)];
        let start = span.start as usize;
        core::str::from_utf8(&self.text[start..start + usize::from(span.length)])
            .expect("copied UTF8")
    }
    pub(super) fn at(&self, index: usize) -> AdmissionReceiptRef<'_> {
        let entry = self.order[index];
        if entry & 128 != 0 {
            return AdmissionReceiptRef::from(&self.hot[usize::from(entry & 127)]);
        }
        let row = &self.rows[usize::from(entry)];
        AdmissionReceiptRef {
            admission_id: ReceiptIdentityRef::Hex(&row.admission),
            credential: ReceiptCredentialRef {
                credential_id: ReceiptIdentityRef::Hex(&row.credential),
                body_id: self.text(row.texts[0]),
                part_id: self.text(row.texts[1]),
                host_id: self.text(row.texts[2]),
                boot_id: self.text(row.texts[3]),
                issued_at_millis: row.issued,
            },
        }
    }
    fn intern(&mut self, text: &str) -> u16 {
        if let Some(index) = (0..self.spans.len()).find(|index| self.text(*index as u16) == text) {
            return index as u16;
        }
        let index = self.spans.len() as u16;
        self.spans.push(Span {
            start: self.text.len() as u32,
            length: text.len() as u16,
        });
        self.text.extend_from_slice(text.as_bytes());
        index
    }
    pub(super) fn usage(&self) -> ReceiptLogStorageUsage {
        ReceiptLogStorageUsage {
            owned_receipts: self.hot.len(),
            historical_receipts: self.rows.len(),
            allocation_capacities: [
                self.hot.capacity() * core::mem::size_of::<AdmissionReceipt>(),
                self.rows.capacity() * core::mem::size_of::<Row>(),
                self.order.capacity(),
                self.text.capacity(),
                self.spans.capacity() * core::mem::size_of::<Span>(),
            ],
        }
    }
    pub(super) fn representation_bytes(&self) -> usize {
        self.hot.capacity() * core::mem::size_of::<AdmissionReceipt>()
            + self.rows.capacity() * core::mem::size_of::<Row>()
            + self.order.capacity()
            + self.text.capacity()
            + self.spans.capacity() * core::mem::size_of::<Span>()
    }
    pub(super) fn try_push(&mut self, receipt: AdmissionReceipt) -> Result<(), AdmissionRefusal> {
        // Both replacements precede movement, so refusal preserves old capacities.
        let mut hot = Vec::new();
        let mut order = Vec::new();
        hot.try_reserve_exact(self.hot.len() + 1)
            .map_err(|_| AdmissionRefusal::ReceiptCapacityExhausted)?;
        order
            .try_reserve_exact(self.order.len() + 1)
            .map_err(|_| AdmissionRefusal::ReceiptCapacityExhausted)?;
        order.extend_from_slice(&self.order);
        order.push(128 | self.hot.len() as u8);
        hot.append(&mut self.hot);
        hot.push(receipt);
        self.hot = hot;
        self.order = order;
        Ok(())
    }
}
pub(super) struct Prepared {
    compact: Compact,
    keep: [bool; MAX_ADMISSION_RECEIPTS],
}
impl Prepared {
    pub(super) fn finish(mut self, prior: Storage) -> Compact {
        let receipts = match prior {
            Storage::Owned(v) => v,
            Storage::Compact(v) => v.hot,
        };
        for (index, receipt) in receipts.into_iter().enumerate() {
            if self.keep[index] {
                self.compact.hot.push(receipt);
            }
        }
        self.compact
    }
}
fn reserve<T>(count: usize) -> Result<Vec<T>, AdmissionStorageCompactionRefusal> {
    let mut v = Vec::new();
    v.try_reserve_exact(count)
        .map_err(|_| AdmissionStorageCompactionRefusal::Capacity)?;
    Ok(v)
}
fn source_hot(log: &AdmissionReceiptLog, index: usize) -> Option<usize> {
    match &log.0 {
        Storage::Owned(_) => Some(index),
        Storage::Compact(v) => {
            let entry = v.order[index];
            (entry & 128 != 0).then_some(usize::from(entry & 127))
        }
    }
}
fn hot(log: &AdmissionReceiptLog, index: usize) -> bool {
    let receipt = log.at(index);
    receipt.digests().is_none()
        || log
            .iter()
            .skip(index + 1)
            .all(|later| later.credential.part_id != receipt.credential.part_id)
}
fn earlier_text(
    log: &AdmissionReceiptLog,
    cold: &[bool; MAX_ADMISSION_RECEIPTS],
    index: usize,
    field: usize,
    text: &str,
) -> bool {
    for prior in 0..=index {
        if !cold[prior] {
            continue;
        }
        let fields = log.at(prior).texts();
        let end = if prior == index { field } else { 4 };
        if fields[..end].contains(&text) {
            return true;
        }
    }
    false
}
pub(super) fn prepare(
    log: &AdmissionReceiptLog,
) -> Result<Option<Prepared>, AdmissionStorageCompactionRefusal> {
    if log.len() > MAX_ADMISSION_RECEIPTS {
        return Err(AdmissionStorageCompactionRefusal::Capacity);
    }
    let mut cold = [false; MAX_ADMISSION_RECEIPTS];
    for (index, flag) in cold.iter_mut().enumerate().take(log.len()) {
        *flag = !hot(log, index);
    }
    let mut hot_count = 0;
    let mut cold_count = 0;
    let mut text_bytes = 0;
    let mut text_count = 0;
    let mut keep = [false; MAX_ADMISSION_RECEIPTS];
    for index in 0..log.len() {
        if !cold[index] {
            // A cold row is never latest: mutable access to historical rows is
            // absent, and appends retain their typed latest credential.
            let source =
                source_hot(log, index).ok_or(AdmissionStorageCompactionRefusal::Capacity)?;
            keep[source] = true;
            hot_count += 1;
        } else {
            cold_count += 1;
            for (field, text) in log.at(index).texts().iter().enumerate() {
                if text.len() > crate::MAX_LIFECYCLE_ID_BYTES {
                    return Err(AdmissionStorageCompactionRefusal::Capacity);
                }
                if !earlier_text(log, &cold, index, field, text) {
                    text_bytes += text.len();
                    text_count += 1;
                }
            }
        }
    }
    if cold_count == 0 {
        return Ok(None);
    }
    if let Storage::Compact(previous) = &log.0 {
        if previous.hot.len() == hot_count && previous.rows.len() == cold_count {
            return Ok(None);
        }
    }
    let mut compact = Compact {
        hot: reserve(hot_count)?,
        rows: reserve(cold_count)?,
        order: reserve(log.len())?,
        text: reserve(text_bytes)?,
        spans: reserve(text_count)?,
    };
    let mut next_hot = 0;
    for index in 0..log.len() {
        if !cold[index] {
            compact.order.push(128 | next_hot);
            next_hot += 1;
        } else {
            let receipt = log.at(index);
            let (admission, credential) = receipt.digests().expect("checked canonical encoding");
            let texts = receipt.texts().map(|text| compact.intern(text));
            compact.order.push(compact.rows.len() as u8);
            compact.rows.push(Row {
                admission,
                credential,
                texts,
                issued: receipt.credential.issued_at_millis,
            });
        }
    }
    Ok(Some(Prepared { compact, keep }))
}
