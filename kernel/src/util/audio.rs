use core::num::NonZeroU8;

#[repr(u32)]
#[derive(Copy, Clone)]
pub enum Pitch {
    C = 65,
    Cs = 69,
    D = 73,
    Ds = 78,
    E = 82,
    F = 87,
    Fs = 92,
    G = 98,
    Gs = 104,
    A = 110,
    As = 117,
    B = 123,
}

impl Pitch {
    pub fn to_freq(&self, octave: NonZeroU8) -> u32 {
        let base_freq = *self as u32;
        let octave_mul = 1u32 << (octave.get() - 1);
        base_freq * octave_mul
    }
}
