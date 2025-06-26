use crate::{
    arch::{addr::VirtualAddress, mmio::IoBox, pin::IntoPinnedMutableSlice, volatile::Volatile},
    device::xhc::{
        context::OutputContext,
        trb::{GenericTrbEntry, TrbRing, TrbType},
    },
    error::{Error, Result},
    util::mutex::Mutex,
};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    marker::PhantomPinned,
    mem::MaybeUninit,
    ops::Range,
    pin::Pin,
    ptr::{read_volatile, write_volatile},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbMode {
    Unknown(u32),
    FullSpeed,
    LowSpeed,
    HighSpeed,
    SuperSpeed,
}

impl UsbMode {
    pub fn psi(&self) -> u32 {
        match *self {
            UsbMode::Unknown(psi) => psi,
            UsbMode::FullSpeed => 0,
            UsbMode::LowSpeed => 1,
            UsbMode::HighSpeed => 2,
            UsbMode::SuperSpeed => 3,
        }
    }
}

#[repr(C)]
pub struct CapabilityRegisters {
    pub cap_reg_len: Volatile<u8>,
    reserved: Volatile<u8>,
    pub interface_ver_num: Volatile<u16>,
    hcs_params1: Volatile<u32>,
    pub hcs_params2: Volatile<u32>,
    pub hcs_params3: Volatile<u32>,
    pub hcc_params1: Volatile<u32>,
    db_offset: Volatile<u32>,
    rts_offset: Volatile<u32>,
    pub hcc_params2: Volatile<u32>,
}

impl CapabilityRegisters {
    pub fn cap_reg_len(&self) -> usize {
        self.cap_reg_len.read() as usize
    }

    pub fn db_offset(&self) -> usize {
        self.db_offset.read() as usize
    }

    pub fn rts_offset(&self) -> usize {
        self.rts_offset.read() as usize
    }

    pub fn num_of_device_slots(&self) -> usize {
        let hcs_params1 = self.hcs_params1.read();
        (hcs_params1 & 0xff) as usize
    }

    pub fn num_of_ports(&self) -> usize {
        let hcs_params1 = self.hcs_params1.read();
        (hcs_params1 & 0xff) as usize
    }

    pub fn num_scratchpad_bufs(&self) -> usize {
        let hcs_params2 = self.hcs_params2.read();
        (((hcs_params2 & 0xf_8000) >> 16) | ((hcs_params2 & 0x7c00_0000) >> 26)) as usize
    }
}

#[repr(C, align(64))]
pub struct DeviceContextBaseAddressArrayInner {
    scratchpad_table_ptr: *const *const u8,
    context: [u64; 255],
    _pinned: PhantomPinned,
}

impl DeviceContextBaseAddressArrayInner {
    pub fn new() -> Self {
        unsafe { MaybeUninit::zeroed().assume_init() }
    }
}

pub struct DeviceContextBaseAddressArray {
    inner: Pin<Box<DeviceContextBaseAddressArrayInner>>,
    context: [Option<Pin<Box<OutputContext>>>; 255],
    scratchpad_bufs: ScratchpadBuffers,
}

impl DeviceContextBaseAddressArray {
    pub fn new(scratchpad_bufs: ScratchpadBuffers) -> Self {
        let mut inner = DeviceContextBaseAddressArrayInner::new();
        inner.scratchpad_table_ptr = scratchpad_bufs.table.as_ref().as_ptr();

        Self {
            inner: Box::pin(inner),
            context: [(); 255].map(|_| None),
            scratchpad_bufs,
        }
    }

    pub fn inner_mut_ptr(&self) -> *const DeviceContextBaseAddressArrayInner {
        self.inner.as_ref().get_ref()
    }

    pub fn set_output_context(
        &mut self,
        slot: u8,
        output_context: Pin<Box<OutputContext>>,
    ) -> Result<()> {
        let index = slot as usize - 1;
        self.context[index] = Some(output_context);
        unsafe {
            self.inner.as_mut().get_unchecked_mut().context[index] =
                self.context[index]
                    .as_ref()
                    .ok_or(Error::Failed("Output context not set"))?
                    .as_ref()
                    .get_ref() as *const _ as u64;
        }
        Ok(())
    }
}

pub struct UsbCommandRegister(Volatile<u32>);

impl UsbCommandRegister {
    fn read(&self) -> u32 {
        self.0.read()
    }

    fn write(&mut self, value: u32) {
        self.0.write(value);
    }

    pub fn set_run_stop(&mut self, value: bool) {
        self.write((self.read() & !0x1) | (value as u32));
    }

    pub fn host_controller_reset(&self) -> bool {
        (self.read() & 0x2) != 0
    }

    pub fn set_host_controller_reset(&mut self, value: bool) {
        self.write((self.read() & !0x2) | ((value as u32) << 1));
    }

    pub fn set_intr_enable(&mut self, value: bool) {
        self.write((self.read() & !0x4) | ((value as u32) << 2));
    }
}

pub struct UsbStatusRegister(Volatile<u32>);

impl UsbStatusRegister {
    pub fn read(&self) -> u32 {
        self.0.read()
    }

    fn write(&mut self, value: u32) {
        self.0.write(value);
    }

    pub fn hchalted(&self) -> bool {
        (self.read() & 0x1) != 0
    }

    pub fn host_system_err(&self) -> bool {
        (self.read() & 0x4) != 0
    }

    pub fn set_host_system_err(&mut self, value: bool) {
        self.write((self.read() & !0x4) | ((value as u32) << 2));
    }

    pub fn event_int(&self) -> bool {
        (self.read() & 0x8) != 0
    }

    pub fn set_event_int(&mut self, value: bool) {
        self.write((self.read() & !0x8) | ((value as u32) << 3));
    }

    pub fn port_change_detect(&self) -> bool {
        (self.read() & 0x10) != 0
    }

    pub fn set_port_change_detect(&mut self, value: bool) {
        self.write((self.read() & !0x10) | ((value as u32) << 4));
    }

    pub fn save_restore_err(&self) -> bool {
        (self.read() & 0x400) != 0
    }

    pub fn set_save_restore_err(&mut self, value: bool) {
        self.write((self.read() & !0x400) | ((value as u32) << 10));
    }

    pub fn controller_not_ready(&self) -> bool {
        (self.read() & 0x800) != 0
    }

    pub fn host_controller_err(&self) -> bool {
        (self.read() & 0x1000) != 0
    }
}

#[repr(C)]
pub struct OperationalRegisters {
    pub usb_cmd: UsbCommandRegister,
    pub usb_status: UsbStatusRegister,
    pub page_size: Volatile<u32>,
    reserved0: [u32; 2],
    pub dn_ctrl: Volatile<u32>,
    pub cmd_ring_ctrl: Volatile<u64>,
    reserved1: [u64; 2],
    pub dcbaa_ptr: Volatile<*const DeviceContextBaseAddressArrayInner>,
    pub config: Volatile<u64>,
}

impl OperationalRegisters {
    pub fn set_max_device_slots_enabled(&mut self, value: u8) {
        self.config
            .write((self.config.read() & !0xff) | (value as u64));
    }

    pub fn set_cmd_ring_ctrl(&mut self, ring: &mut CommandRing) {
        let cycle_state = 1;
        self.cmd_ring_ctrl
            .write(ring.ring_phys_addr() | cycle_state);
    }
}

#[repr(C)]
struct InterrupterRegisterSet {
    manage: u32,
    moderation: u32,
    erst_size: u32,
    rsvdp: u32,
    erst_base: u64,
    erdp: u64,
}

#[repr(C)]
pub struct RuntimeRegisters {
    mfindex: Volatile<u32>,
    reserved: [u32; 7],
    int_reg_set: [InterrupterRegisterSet; 1024],
}

impl RuntimeRegisters {
    pub fn init_int_reg_set(&mut self, index: usize, ring: &mut EventRing) -> Result<()> {
        let int_reg_set = self
            .int_reg_set
            .get_mut(index)
            .ok_or(Error::IndexOutOfBoundsError(index))?;
        int_reg_set.erst_size = 1;
        int_reg_set.erdp = ring.ring_phys_addr();
        int_reg_set.erst_base = ring.erst_phys_addr();
        int_reg_set.manage = 0;
        ring.set_erdp(&mut int_reg_set.erdp as *mut u64);

        Ok(())
    }

    pub fn mfindex(&self) -> usize {
        self.mfindex.read() as usize
    }
}

pub struct ScratchpadBuffers {
    pub table: Pin<Box<[*const u8]>>,
    pub bufs: Vec<Pin<Box<[u8]>>>,
}

#[repr(C, align(4096))]
pub struct EventRingSegmentTableEntry {
    pub ring_seg_base_addr: u64,
    pub ring_seg_size: u16,
    reserved: [u16; 3],
}

impl EventRingSegmentTableEntry {
    pub fn new(ring: &IoBox<TrbRing>) -> Result<IoBox<Self>> {
        let mut erst: IoBox<Self> = IoBox::new();
        {
            let erst = unsafe { erst.get_unchecked_mut() };
            erst.ring_seg_base_addr = ring.as_ref() as *const _ as u64;
            erst.ring_seg_size = ring
                .as_ref()
                .num_trbs()
                .try_into()
                .or(Err(Error::Failed("Too large num trbs")))?;
        }

        Ok(erst)
    }
}

pub struct EventRing {
    ring: IoBox<TrbRing>,
    erst: IoBox<EventRingSegmentTableEntry>,
    cycle_state_ours: bool,
    erdp: Option<*mut u64>,
}

impl EventRing {
    pub fn new() -> Result<Self> {
        let ring = TrbRing::new();
        let erst = EventRingSegmentTableEntry::new(&ring)?;

        Ok(Self {
            ring,
            erst,
            cycle_state_ours: true,
            erdp: None,
        })
    }

    pub fn ring_phys_addr(&self) -> u64 {
        self.ring.as_ref() as *const _ as u64
    }

    pub fn set_erdp(&mut self, erdp: *mut u64) {
        self.erdp = Some(erdp);
    }

    pub fn erst_phys_addr(&self) -> u64 {
        self.erst.as_ref() as *const _ as u64
    }

    pub fn pop(&mut self) -> Result<Option<GenericTrbEntry>> {
        if !self.has_next_event() {
            return Ok(None);
        }

        let trb = self.ring.as_ref().current();
        let trb_ptr = self.ring.as_ref().current_ptr();
        unsafe { self.ring.get_unchecked_mut() }.advance_index_notoggle(self.cycle_state_ours)?;

        unsafe {
            let erdp = self.erdp.ok_or(Error::Failed("ERDP not set"))?;
            write_volatile(erdp, (trb_ptr as u64) | (*erdp & 0b1111));
        }

        if self.ring.as_ref().index() == 0 {
            self.cycle_state_ours = !self.cycle_state_ours;
        }

        Ok(Some(trb))
    }

    fn has_next_event(&self) -> bool {
        let trb_cycle = self.ring.as_ref().current().cycle_state();
        trb_cycle == self.cycle_state_ours
    }
}

pub struct CommandRing {
    ring: IoBox<TrbRing>,
    cycle_state_ours: bool,
}

impl Default for CommandRing {
    fn default() -> Self {
        let mut me = Self {
            ring: TrbRing::new(),
            cycle_state_ours: false,
        };

        let link_trb = GenericTrbEntry::trb_link(me.ring.as_ref());
        unsafe { me.ring.get_unchecked_mut() }
            .write(TrbRing::NUM_TRBS - 1, link_trb)
            .unwrap();

        me
    }
}

impl CommandRing {
    pub fn ring_phys_addr(&self) -> u64 {
        self.ring.as_ref() as *const _ as u64
    }

    pub fn push(&mut self, mut src: GenericTrbEntry) -> Result<u64> {
        let ring = unsafe { self.ring.get_unchecked_mut() };
        if ring.current().cycle_state() != self.cycle_state_ours {
            return Err(Error::Failed("Command ring is full"));
        }

        src.set_cycle_state(self.cycle_state_ours);
        let dst_ptr = ring.current_ptr();
        ring.write_current(src)?;
        ring.advance_index(!self.cycle_state_ours)?;

        if ring.current().trb_type() == TrbType::Link as u32 {
            ring.advance_index(!self.cycle_state_ours)?;
            self.cycle_state_ours = !self.cycle_state_ours;
        }

        Ok(dst_ptr as u64)
    }
}

#[repr(C)]
pub struct PortScEntry {
    ptr: Mutex<*mut u32>,
}

impl PortScEntry {
    pub fn new(ptr: *mut u32) -> Self {
        Self {
            ptr: Mutex::new(ptr),
        }
    }

    fn read(&self) -> u32 {
        let portsc = self.ptr.spin_lock();
        unsafe { read_volatile(*portsc) }
    }

    fn write(&self, value: u32) {
        let portsc = self.ptr.spin_lock();
        unsafe { write_volatile(*portsc, value) };
    }

    // current connect status
    pub fn ccs(&self) -> bool {
        self.read() & 0x1 != 0
    }

    // port power
    pub fn pp(&self) -> bool {
        self.read() & 0x200 != 0
    }

    pub fn set_pp(&self, value: bool) {
        let e_value = self.read();
        self.write((e_value & !0x200) | ((value as u32) << 9));
    }

    // port reset
    pub fn pr(&self) -> bool {
        self.read() & 0x10 != 0
    }

    pub fn set_pr(&self, value: bool) {
        let e_value = self.read();
        self.write((e_value & !0x10) | ((value as u32) << 4));
    }

    // port enabled/disabled
    pub fn ped(&self) -> bool {
        self.read() & 0x2 != 0
    }

    pub fn reset_port(&self) {
        self.set_pp(true);
        while !self.pp() {} // wait

        self.set_pr(true);
        while self.pr() {} // wait
    }

    pub fn is_enabled(&self) -> bool {
        self.pp() && self.ccs() && self.ped() && !self.pr()
    }

    pub fn port_speed(&self) -> UsbMode {
        let value = (self.read() >> 20) & 0x1f;
        match value {
            1 => UsbMode::FullSpeed,
            2 => UsbMode::LowSpeed,
            3 => UsbMode::HighSpeed,
            4 => UsbMode::SuperSpeed,
            v => UsbMode::Unknown(v),
        }
    }

    pub fn max_packet_size(&self) -> Result<u16> {
        match self.port_speed() {
            UsbMode::FullSpeed | UsbMode::LowSpeed => Ok(8),
            UsbMode::HighSpeed => Ok(64),
            UsbMode::SuperSpeed => Ok(512),
            _ => Err(Error::Failed("Unknown Protocol speed ID")),
        }
    }
}

pub struct PortSc {
    entries: Vec<Rc<PortScEntry>>,
}

impl PortSc {
    pub fn new(bar: &VirtualAddress, cap_regs: &CapabilityRegisters) -> Self {
        let base: *mut u32 = bar.offset(cap_regs.cap_reg_len() + 0x400).as_ptr_mut();
        let num_of_ports = cap_regs.num_of_ports();
        let mut entries = Vec::new();
        for port in 1..=num_of_ports {
            let ptr = unsafe { base.add((port - 1) * 4) };
            entries.push(Rc::new(PortScEntry::new(ptr)));
        }

        Self { entries }
    }

    pub fn port_range(&self) -> Range<usize> {
        1..self.entries.len() + 1
    }

    pub fn get(&self, port: usize) -> Option<Rc<PortScEntry>> {
        self.entries.get(port.wrapping_sub(1)).cloned()
    }
}

pub struct Doorbell {
    ptr: Mutex<*mut u32>,
}

impl Doorbell {
    pub fn new(ptr: *mut u32) -> Self {
        Self {
            ptr: Mutex::new(ptr),
        }
    }

    pub fn notify(&self, target: u8, task: u16) {
        let value = (target as u32) | (task as u32) << 16;
        unsafe {
            write_volatile(*self.ptr.spin_lock(), value);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(unused)]
pub enum UsbDescriptorType {
    Device = 1,
    Config = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
}

#[derive(Debug, Clone, Copy, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct UsbDeviceDescriptor {
    pub desc_len: u8,
    pub desc_type: u8,
    pub version: u16,
    pub dev_class: u8,
    pub dev_subclass: u8,
    pub dev_protocol: u8,
    pub max_packet_size: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_version: u16,
    pub manufacturer_index: u8,
    pub product_index: u8,
    pub serial_index: u8,
    pub num_of_config: u8,
}

unsafe impl IntoPinnedMutableSlice for UsbDeviceDescriptor {}
