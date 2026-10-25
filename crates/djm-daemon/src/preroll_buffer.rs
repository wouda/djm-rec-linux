pub struct PrerollBuffer {
    buffer: Vec<(i32, i32)>,
    capacity: usize,
    write_pos: usize,
    is_full: bool,
}

impl PrerollBuffer {
    pub fn new(capacity_samples: usize) -> Self {
        Self {
            buffer: vec![(0, 0); capacity_samples.max(1)],
            capacity: capacity_samples.max(1),
            write_pos: 0,
            is_full: false,
        }
    }

    pub fn push(&mut self, sample: (i32, i32)) {
        self.buffer[self.write_pos] = sample;
        self.write_pos += 1;
        if self.write_pos >= self.capacity {
            self.write_pos = 0;
            self.is_full = true;
        }
    }

    pub fn push_block(&mut self, samples: &[(i32, i32)]) {
        for &s in samples {
            self.push(s);
        }
    }

    /// Returns all buffered samples in chronological order and resets the buffer.
    pub fn drain_chronological(&mut self) -> Vec<(i32, i32)> {
        let mut out = Vec::with_capacity(if self.is_full { self.capacity } else { self.write_pos });

        if self.is_full {
            // Oldest samples are from write_pos..capacity
            out.extend_from_slice(&self.buffer[self.write_pos..self.capacity]);
            // Followed by 0..write_pos
            out.extend_from_slice(&self.buffer[0..self.write_pos]);
        } else {
            out.extend_from_slice(&self.buffer[0..self.write_pos]);
        }

        self.clear();
        out
    }

    pub fn clear(&mut self) {
        self.write_pos = 0;
        self.is_full = false;
    }

    pub fn len(&self) -> usize {
        if self.is_full {
            self.capacity
        } else {
            self.write_pos
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.is_full && self.write_pos == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preroll_drain_order() {
        let mut buf = PrerollBuffer::new(4);
        buf.push((1, 1));
        buf.push((2, 2));
        assert_eq!(buf.drain_chronological(), vec![(1, 1), (2, 2)]);
        assert!(buf.is_empty());

        // Overfill test
        buf.push((10, 10));
        buf.push((20, 20));
        buf.push((30, 30));
        buf.push((40, 40));
        buf.push((50, 50)); // overwrites 10
        buf.push((60, 60)); // overwrites 20

        let drained = buf.drain_chronological();
        assert_eq!(drained, vec![(30, 30), (40, 40), (50, 50), (60, 60)]);
        assert!(buf.is_empty());
    }
}
