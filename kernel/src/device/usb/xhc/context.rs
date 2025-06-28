use crate::{
    arch::volatile::Volatile,
    device::usb::xhc::register::UsbMode,
    error::{Error, Result},
};
use core::{marker::PhantomPinned, mem::MaybeUninit, pin::Pin};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EndpointType {
    IsochOut = 1,
    BulkOut = 2,
    InterruptOut = 3,
    Control = 4,
    IsochIn = 5,
    BulkIn = 6,
    InterruptIn = 7,
}

#[repr(C, align(32))]
#[derive(Default)]
pub struct EndpointContext {
    data: [u32; 2],
    tr_dequeue_ptr: Volatile<u64>,
    ave_trb_len: u16,
    max_esit_payload_low: u16,
    reserved: [u32; 3],
}

impl EndpointContext {
    pub fn new() -> Self {
        unsafe { MaybeUninit::zeroed().assume_init() }
    }

    pub fn new_ctrl_endpoint(max_packet_size: u16, tr_dequeue_ptr: u64) -> Result<Self> {
        let mut eq = Self::new();
        eq.set_ep_type(EndpointType::Control)?;
        eq.set_dequeue_cycle_state(true);
        eq.set_error_count(3)?;
        eq.set_max_packet_size(max_packet_size);
        eq.set_ring_dequeue_ptr(tr_dequeue_ptr);
        eq.ave_trb_len = 8;

        Ok(eq)
    }

    fn set_ring_dequeue_ptr(&mut self, tr_dequeue_ptr: u64) {
        self.tr_dequeue_ptr
            .write(self.tr_dequeue_ptr.read() & 0x1 | (tr_dequeue_ptr & !0x1));
    }

    fn set_max_packet_size(&mut self, max_packet_size: u16) {
        let max_packet_size = max_packet_size as u32;
        self.data[1] &= !(0xffff << 16);
        self.data[1] |= max_packet_size << 16;
    }

    fn set_error_count(&mut self, error_count: u32) -> Result<()> {
        if error_count & !0b11 == 0 {
            self.data[1] &= !(0b11 << 1);
            self.data[1] |= error_count << 1;
            Ok(())
        } else {
            Err(Error::Failed("Invalid error count"))
        }
    }

    fn set_dequeue_cycle_state(&mut self, dcs: bool) {
        self.tr_dequeue_ptr
            .write(self.tr_dequeue_ptr.read() & !0x1 | (dcs as u64));
    }

    fn set_ep_type(&mut self, ep_type: EndpointType) -> Result<()> {
        let raw_ep_type = ep_type as u32;
        if raw_ep_type < 8 {
            self.data[1] &= !(0b111 << 3);
            self.data[1] |= raw_ep_type << 3;
            Ok(())
        } else {
            Err(Error::Failed("Invalid endpoint type"))
        }
    }
}

#[repr(C, align(32))]
#[derive(Default)]
pub struct DeviceContext {
    slot_context: [u32; 8],
    ep_contexts: [EndpointContext; 2 * 15 + 1],
    _pinned: PhantomPinned,
}

impl DeviceContext {
    pub fn set_port_speed(&mut self, mode: UsbMode) -> Result<()> {
        if mode.psi() < 16u32 {
            self.slot_context[0] &= !(0xf << 20);
            self.slot_context[0] |= mode.psi() << 20;
            Ok(())
        } else {
            Err(Error::Failed("Psi out of range"))
        }
    }

    pub fn set_last_valid_dci(&mut self, dci: usize) -> Result<()> {
        if dci <= 31 {
            self.slot_context[0] &= !(0x1f << 27);
            self.slot_context[0] |= (dci as u32) << 27;
            Ok(())
        } else {
            Err(Error::Failed("DCI out of range"))
        }
    }

    pub fn set_root_hub_port_num(&mut self, port: usize) -> Result<()> {
        if 0 < port && port < 256 {
            self.slot_context[1] &= !(0xff << 16);
            self.slot_context[1] |= (port as u32) << 16;
            Ok(())
        } else {
            Err(Error::Failed("Port number out of range"))
        }
    }
}

#[repr(C, align(4096))]
#[derive(Default)]
pub struct OutputContext {
    device_context: DeviceContext,
    _pinned: PhantomPinned,
}

#[repr(C, align(32))]
#[derive(Default)]
pub struct InputControlContext {
    drop_context_bitmap: u32,
    add_context_bitmap: u32,
    data: [u32; 6],
    _pinned: PhantomPinned,
}

impl InputControlContext {
    pub fn add_context(&mut self, ici: usize) -> Result<()> {
        if ici < 32 {
            self.add_context_bitmap |= 1 << ici;
            Ok(())
        } else {
            Err(Error::Failed("ICI out of range"))
        }
    }
}

#[repr(C, align(4096))]
#[derive(Default)]
pub struct InputContext {
    input_ctrl_context: InputControlContext,
    device_context: DeviceContext,
    _pinned: PhantomPinned,
}

impl InputContext {
    pub fn set_ep_context(self: &mut Pin<&mut Self>, dci: usize, ep_context: EndpointContext) {
        unsafe {
            self.as_mut().get_unchecked_mut().device_context.ep_contexts[dci - 1] = ep_context;
        }
    }

    pub fn set_input_ctrl_context(
        self: &mut Pin<&mut Self>,
        input_ctrl_context: InputControlContext,
    ) {
        unsafe {
            self.as_mut().get_unchecked_mut().input_ctrl_context = input_ctrl_context;
        }
    }

    pub fn set_port_speed(self: &mut Pin<&mut Self>, mode: UsbMode) -> Result<()> {
        unsafe {
            self.as_mut()
                .get_unchecked_mut()
                .device_context
                .set_port_speed(mode)
        }
    }

    pub fn set_root_hub_port_num(self: &mut Pin<&mut Self>, port: usize) -> Result<()> {
        unsafe {
            self.as_mut()
                .get_unchecked_mut()
                .device_context
                .set_root_hub_port_num(port)
        }
    }

    pub fn set_last_valid_dci(self: &mut Pin<&mut Self>, dci: usize) -> Result<()> {
        unsafe {
            self.as_mut()
                .get_unchecked_mut()
                .device_context
                .set_last_valid_dci(dci)
        }
    }
}
