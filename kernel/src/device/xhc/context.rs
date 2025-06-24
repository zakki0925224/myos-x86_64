use crate::arch::volatile::Volatile;
use core::marker::PhantomPinned;

#[repr(C, align(32))]
pub struct EndpointContext {
    data: [u32; 2],
    tr_dequeue_ptr: Volatile<u64>,
    ave_trb_len: u16,
    max_esit_payload_low: u16,
    reserved: [u32; 3],
}

#[repr(C, align(32))]
pub struct DeviceContext {
    slot_context: [u32; 8],
    ep_contexts: [EndpointContext; 2 * 15 + 1],
}

#[repr(C, align(4096))]
pub struct OutputContext {
    device_context: DeviceContext,
    _pinned: PhantomPinned,
}
