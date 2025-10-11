#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Rflags(u64);

impl From<u64> for Rflags {
    fn from(value: u64) -> Self {
        Self(value | (1 << 1)) // always 1
    }
}

impl core::fmt::Debug for Rflags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Rflags")
            .field("CF", &self.cf())
            .field("PF", &self.pf())
            .field("AF", &self.af())
            .field("ZF", &self.zf())
            .field("SF", &self.sf())
            .field("TF", &self.tf())
            .field("IF", &self.if_())
            .field("DF", &self.df())
            .field("OF", &self.of())
            .field("IOPL", &self.iopl())
            .field("NT", &self.nt())
            .field("RF", &self.rf())
            .field("VM", &self.vm())
            .field("AC", &self.ac())
            .field("VIF", &self.vif())
            .field("VIP", &self.vip())
            .field("ID", &self.id())
            .finish()
    }
}

impl Rflags {
    const BIT_MASK_CF: u64 = 1 << 0;
    const BIT_MASK_PF: u64 = 1 << 2;
    const BIT_MASK_AF: u64 = 1 << 4;
    const BIT_MASK_ZF: u64 = 1 << 6;
    const BIT_MASK_SF: u64 = 1 << 7;
    const BIT_MASK_TF: u64 = 1 << 8;
    const BIT_MASK_IF: u64 = 1 << 9;
    const BIT_MASK_DF: u64 = 1 << 10;
    const BIT_MASK_OF: u64 = 1 << 11;
    const BIT_MASK_IOPL: u64 = 0b11 << 12;
    const BIT_MASK_NT: u64 = 1 << 14;
    const BIT_MASK_RF: u64 = 1 << 16;
    const BIT_MASK_VM: u64 = 1 << 17;
    const BIT_MASK_AC: u64 = 1 << 18;
    const BIT_MASK_VIF: u64 = 1 << 19;
    const BIT_MASK_VIP: u64 = 1 << 20;
    const BIT_MASK_ID: u64 = 1 << 21;

    pub const fn default() -> Self {
        Self(1 << 1) // always 1
    }

    pub fn cf(&self) -> bool {
        (self.0 & Self::BIT_MASK_CF) != 0
    }

    pub fn set_cf(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_CF) | ((value as u64) << 0);
    }

    pub fn pf(&self) -> bool {
        (self.0 & Self::BIT_MASK_PF) != 0
    }

    pub fn set_pf(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_PF) | ((value as u64) << 2);
    }

    pub fn af(&self) -> bool {
        (self.0 & Self::BIT_MASK_AF) != 0
    }

    pub fn set_af(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_AF) | ((value as u64) << 4);
    }

    pub fn zf(&self) -> bool {
        (self.0 & Self::BIT_MASK_ZF) != 0
    }

    pub fn set_zf(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_ZF) | ((value as u64) << 6);
    }

    pub fn sf(&self) -> bool {
        (self.0 & Self::BIT_MASK_SF) != 0
    }

    pub fn set_sf(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_SF) | ((value as u64) << 7);
    }

    pub fn tf(&self) -> bool {
        (self.0 & Self::BIT_MASK_TF) != 0
    }

    pub fn set_tf(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_TF) | ((value as u64) << 8);
    }

    pub fn if_(&self) -> bool {
        (self.0 & Self::BIT_MASK_IF) != 0
    }

    pub fn set_if_(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_IF) | ((value as u64) << 9);
    }

    pub fn df(&self) -> bool {
        (self.0 & Self::BIT_MASK_DF) != 0
    }

    pub fn set_df(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_DF) | ((value as u64) << 10);
    }

    pub fn of(&self) -> bool {
        (self.0 & Self::BIT_MASK_OF) != 0
    }

    pub fn set_of(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_OF) | ((value as u64) << 11);
    }

    pub fn iopl(&self) -> u8 {
        (((self.0 & Self::BIT_MASK_IOPL) >> 12) as u8) & 0b11
    }

    pub fn set_iopl(&mut self, value: u8) {
        self.0 = (self.0 & !Self::BIT_MASK_IOPL) | (((value as u64) & 0b11) << 12);
    }

    pub fn nt(&self) -> bool {
        (self.0 & Self::BIT_MASK_NT) != 0
    }

    pub fn set_nt(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_NT) | ((value as u64) << 14);
    }

    pub fn rf(&self) -> bool {
        (self.0 & Self::BIT_MASK_RF) != 0
    }

    pub fn set_rf(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_RF) | ((value as u64) << 16);
    }

    pub fn vm(&self) -> bool {
        (self.0 & Self::BIT_MASK_VM) != 0
    }

    pub fn set_vm(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_VM) | ((value as u64) << 17);
    }

    pub fn ac(&self) -> bool {
        (self.0 & Self::BIT_MASK_AC) != 0
    }

    pub fn set_ac(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_AC) | ((value as u64) << 18);
    }

    pub fn vif(&self) -> bool {
        (self.0 & Self::BIT_MASK_VIF) != 0
    }

    pub fn set_vif(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_VIF) | ((value as u64) << 19);
    }

    pub fn vip(&self) -> bool {
        (self.0 & Self::BIT_MASK_VIP) != 0
    }

    pub fn set_vip(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_VIP) | ((value as u64) << 20);
    }

    pub fn id(&self) -> bool {
        (self.0 & Self::BIT_MASK_ID) != 0
    }

    pub fn set_id(&mut self, value: bool) {
        self.0 = (self.0 & !Self::BIT_MASK_ID) | ((value as u64) << 21);
    }
}
