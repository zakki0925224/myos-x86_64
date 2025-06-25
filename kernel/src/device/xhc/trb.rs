use core::{marker::PhantomPinned, pin::Pin, ptr::{read_volatile, write_volatile}};
use crate::{arch::{mmio::IoBox, volatile::Volatile}, device::xhc::context::InputContext, error::{Error, Result}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)]
#[repr(u32)]
pub enum TrbType {
    Normal = 1,
    SetupStage = 2,
    DataStage = 3,
    StatusStage = 4,
    Link = 6,
    EnableSlotCommand = 9,
    AddressDeviceCommand = 11,
    ConfigureEndpointCommand = 12,
    EvaluateContextCommand = 13,
    NoOpCommand = 23,
    TransferEvent = 32,
    CommandCompletionEvent = 33,
    PortStatusChangeEvent = 34,
    HostControllerEvent = 37,
}

#[derive(Default, Clone)]
#[repr(C, align(16))]
pub struct GenericTrbEntry {
    data: Volatile<u64>,
    option: Volatile<u32>,
    ctrl: Volatile<u32>,
}

impl GenericTrbEntry {
    pub fn trb_link(ring: &TrbRing) -> Self {
        let mut trb = GenericTrbEntry::default();
        trb.set_trb_type(TrbType::Link);
        trb.data.write(ring.phys_addr());
        trb.set_toggle_cycle(true);
        trb
    }

    pub fn trb_enable_slot_cmd() -> Self {
        let mut trb = Self::default();
        trb.set_trb_type(TrbType::EnableSlotCommand);
        trb
    }

    pub fn trb_cmd_address_device(input_context: Pin<&InputContext>, slot: u8) -> Self {
        let mut trb = Self::default();
        trb.set_trb_type(TrbType::AddressDeviceCommand);
        trb.data.write(input_context.get_ref() as *const InputContext as u64);
        trb.set_slot_id(slot);
        trb
    }

    pub fn completion_code(&self) -> u32 {
        (self.option.read() >> 24) & 0xff
    }

    pub fn cmd_result_ok(&self) -> Result<()> {
        if self.trb_type() != TrbType::CommandCompletionEvent as u32 {
            Err(Error::Failed("Not a command completion event TRB"))
        }
        else if self.completion_code() != 1 {
            Err(Error::Failed("Command completion code was not success"))
        } else {
            Ok(())
        }
    }

    pub fn set_trb_type(&mut self, trb_type: TrbType) {
        self.ctrl.write(self.ctrl.read() & !0xfc00 | ((trb_type as u32) << 10));
    }

    pub fn set_cycle_state(&mut self, cycle: bool) {
        self.ctrl.write(self.ctrl.read() & !0x1 | (cycle as u32));
    }

    pub fn set_toggle_cycle(&mut self, value: bool) {
        self.ctrl.write(self.ctrl.read() & !0x2 | (value as u32));
    }

    pub fn data(&self) -> u64 {
        self.data.read()
    }

    pub fn slot_id(&self) -> u8 {
        (self.ctrl.read() >> 24) as u8
    }

    pub fn set_slot_id(&mut self, slot_id: u8) {
        self.ctrl.write((self.ctrl.read() & !(0xff << 24)) | ((slot_id as u32) << 24));
    }

    pub fn trb_type(&self) -> u32 {
        (self.ctrl.read() >> 10) & 0x3f
    }

    pub fn cycle_state(&self) -> bool {
        self.ctrl.read() & 0x1 != 0
    }
}

#[repr(C, align(4096))]
pub struct TrbRing {
    trb: [GenericTrbEntry; Self::NUM_TRBS],
    index: usize,
    _pinned: PhantomPinned,
}

impl TrbRing {
    pub const NUM_TRBS: usize = 16;

    pub fn new() -> IoBox<Self> {
        IoBox::new()
    }

    pub const fn num_trbs(&self) -> usize {
        Self::NUM_TRBS
    }

    pub fn write(&mut self, index: usize, trb: GenericTrbEntry) -> Result<()> {
        if index < self.trb.len() {
            unsafe {
                write_volatile(&mut self.trb[index], trb);
            }

            Ok(())
        } else {
            Err(Error::IndexOutOfBoundsError(index))
        }
    }

    pub fn phys_addr(&self) -> u64 {
        &self.trb[0] as *const _ as u64
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn advance_index_notoggle(&mut self, cycle_ours: bool) -> Result<()> {
        if self.current().cycle_state() != cycle_ours {
            return Err(Error::Failed("Invalid cycle state"));
        }

        self.index = (self.index + 1) % self.trb.len();
        Ok(())
    }

    pub fn advance_index(&mut self, new_cycle: bool) -> Result<()> {
        if self.current().cycle_state() == new_cycle {
            return Err(Error::Failed("Invalid cycle state"));
        }

        self.trb[self.index].set_cycle_state(new_cycle);
        self.index = (self.index + 1) % self.trb.len();
        Ok(())
    }

    pub fn current(&self) -> GenericTrbEntry {
        unsafe {
            read_volatile(&self.trb[self.index])
        }
    }

    pub fn write_current(&mut self, trb: GenericTrbEntry) -> Result<()> {
        self.write(self.index, trb);
        Ok(())
    }

    pub fn current_ptr(&self) -> *const GenericTrbEntry {
        &self.trb[self.index] as *const _
    }
}
