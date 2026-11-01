pub(super) struct Ring<const N: usize> {
    bytes: [u8; N],
    head: usize,
    used: usize,
}
impl<const N: usize> Ring<N> {
    pub(super) const fn new() -> Self {
        assert!(N > 0);
        Self {
            bytes: [0; N],
            head: 0,
            used: 0,
        }
    }
    pub(super) fn len(&self) -> usize {
        self.used
    }
    pub(super) fn room(&self) -> usize {
        N - self.used
    }
    pub(super) fn push(&mut self, input: &[u8]) -> usize {
        let count = input.len().min(self.room());
        for (offset, byte) in input[..count].iter().enumerate() {
            self.bytes[(self.head + self.used + offset) % N] = *byte;
        }
        self.used += count;
        count
    }
    pub(super) fn copy(&self, output: &mut [u8]) -> usize {
        let count = output.len().min(self.used);
        for (offset, byte) in output[..count].iter_mut().enumerate() {
            *byte = self.bytes[(self.head + offset) % N];
        }
        count
    }
    pub(super) fn consume(&mut self, count: usize) -> bool {
        if count > self.used {
            return false;
        }
        for offset in 0..count {
            self.bytes[(self.head + offset) % N] = 0;
        }
        self.head = (self.head + count) % N;
        self.used -= count;
        true
    }
    pub(super) fn clear(&mut self) {
        self.bytes.fill(0);
        self.head = 0;
        self.used = 0;
    }
}
