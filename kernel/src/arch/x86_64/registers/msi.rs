#[derive(Debug, Clone, Copy, Default)]
#[repr(transparent)]
pub struct MsiMessageAddressField(u32);

impl MsiMessageAddressField {
    pub fn new(dest_mode: bool, redirection_hint_indication: bool, dest_id: u8) -> Self {
        let mut field;

        field = (dest_mode as u32) << 2;
        field = (field & !0x8) | ((redirection_hint_indication as u32) << 3);
        field = (field & !0xf_f000) | ((dest_id as u32) << 12);
        field = (field & !0xfff0_0000) | (0xfee << 20);

        Self(field)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum DeliveryMode {
    Fixed = 0,
    LowestPriority = 1,
    Smi = 2,
    Nmi = 4,
    Init = 5,
    ExtInt = 7,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Level {
    Deassert = 0,
    Assert = 1,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum TriggerMode {
    Edge = 0,
    Level = 1,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(transparent)]
pub struct MsiMessageDataField(u32);

impl MsiMessageDataField {
    pub fn new(
        vector: u8,
        delivery_mode: DeliveryMode,
        level: Level,
        trigger_mode: TriggerMode,
    ) -> Self {
        let mut field;

        field = vector as u32;
        field = (field & !0x700) | ((delivery_mode as u32) << 8);
        field = (field & !0x4000) | ((level as u32) << 11);
        field = (field & !0x8000) | ((trigger_mode as u32) << 12);

        Self(field)
    }
}
