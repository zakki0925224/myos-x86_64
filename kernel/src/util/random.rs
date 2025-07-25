use alloc::vec::Vec;

fn xorshift64(seed: u64) -> u64 {
    let mut x = seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

fn pcg32(state: &mut u64, inc: u64) -> u32 {
    let oldstate = *state;
    *state = oldstate
        .wrapping_mul(6364136223846793005u64)
        .wrapping_add(inc | 1);
    let xorshifted = (((oldstate >> 18) ^ oldstate) >> 27) as u32;
    let rot = (oldstate >> 59) as u32;
    (xorshifted >> rot) | (xorshifted << ((!rot).wrapping_add(1) & 31))
}

struct XorShift64 {
    seed: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            seed: if seed == 0 { 1 } else { seed },
        }
    }

    fn next(&mut self) -> u64 {
        self.seed = xorshift64(self.seed);
        self.seed
    }

    fn next_bytes(&mut self, buf: &mut [u8]) {
        // skip
        for _ in 0..8 {
            let _ = self.next();
        }

        let mut i = 0;
        while i < buf.len() {
            let value = self.next();
            let bytes = value.to_le_bytes();

            for &byte in &bytes {
                if i < buf.len() {
                    buf[i] = byte;
                    i += 1;
                } else {
                    break;
                }
            }
        }
    }
}

struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    fn new(seed: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (seed << 1) | 1,
        };
        rng.state = seed.wrapping_add(rng.inc);
        let _ = pcg32(&mut rng.state, rng.inc);
        rng
    }

    fn next(&mut self) -> u32 {
        pcg32(&mut self.state, self.inc)
    }

    fn next_bytes(&mut self, buf: &mut [u8]) {
        let mut i = 0;
        while i < buf.len() {
            let value = self.next();
            let bytes = value.to_le_bytes();

            for &byte in &bytes {
                if i < buf.len() {
                    buf[i] = byte;
                    i += 1;
                } else {
                    break;
                }
            }
        }
    }
}

pub fn random_bytes_xorshift64(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = XorShift64::new(seed);
    let mut bytes = Vec::with_capacity(len);
    bytes.resize(len, 0);
    rng.next_bytes(&mut bytes);
    bytes
}

pub fn random_bytes_pcg32(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = Pcg32::new(seed);
    let mut bytes = Vec::with_capacity(len);
    bytes.resize(len, 0);
    rng.next_bytes(&mut bytes);
    bytes
}
