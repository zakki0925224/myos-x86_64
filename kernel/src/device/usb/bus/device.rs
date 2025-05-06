use super::descriptor::{
    config::ConfigurationDescriptor, device::DeviceDescriptor, endpoint::EndpointDescriptor,
    hid::HumanInterfaceDeviceDescriptor, interface::InterfaceDescriptor, Descriptor,
    DescriptorHeader, DescriptorType,
};
use crate::{
    addr::VirtualAddress,
    arch::{mmio::Mmio, volatile::Volatile},
    device::{
        self,
        usb::{
            hid_keyboard::InputData,
            trb::*,
            xhc::{
                context::{endpoint::*, input::InputControlContext},
                ringbuf::*,
            },
        },
    },
    error::Result,
    mem::bitmap::{self, MemoryFrameInfo},
};
use alloc::vec::Vec;
use core::mem::size_of;

const RING_BUF_LEN: usize = 16;
const DEFAULT_CTRL_PIPE_ID: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UsbDeviceError {
    XhcPortNotFoundError,
    InvalidTransferRequestBlockTypeError,
    InvalidRequestError,
}

#[derive(Debug, Clone)]
pub struct UsbDevice {
    pub is_configured: bool,

    slot_id: usize,
    transfer_ring_bufs: [Option<RingBuffer<RING_BUF_LEN>>; 32],
    dev_desc_buf_mem_info: MemoryFrameInfo,
    conf_desc_buf_mem_info: MemoryFrameInfo,

    max_packet_size: u16,

    configured_endpoint_dci: Vec<usize>, // dci, data_buf_virt_addr
    current_conf_index: usize,
    dev_desc: DeviceDescriptor,
    conf_descs: Vec<Descriptor>,
}

impl UsbDevice {
    pub fn new(
        slot_id: usize,
        max_packet_size: u16,
        mut dcp_ring_buf: RingBuffer<RING_BUF_LEN>,
    ) -> Result<Self> {
        dcp_ring_buf.set_link_trb()?;

        let dev_desc_buf_mem_info = bitmap::alloc_mem_frame(1)?;
        let conf_desc_buf_mem_info = bitmap::alloc_mem_frame(1)?;
        bitmap::mem_clear(&dev_desc_buf_mem_info)?;
        bitmap::mem_clear(&conf_desc_buf_mem_info)?;

        let mut transfer_ring_bufs: [Option<RingBuffer<RING_BUF_LEN>>; 32] = Default::default();
        transfer_ring_bufs[1] = Some(dcp_ring_buf);

        let device = Self {
            is_configured: false,
            slot_id,
            transfer_ring_bufs,
            dev_desc_buf_mem_info,
            conf_desc_buf_mem_info,
            max_packet_size,
            configured_endpoint_dci: Vec::new(),
            current_conf_index: 0,
            dev_desc: DeviceDescriptor::default(),
            conf_descs: Vec::new(),
        };

        Ok(device)
    }

    pub fn init(&mut self) -> Result<()> {
        self.request_to_get_desc(DescriptorType::Device, 0)
    }

    pub fn slot_id(&self) -> usize {
        self.slot_id
    }

    pub fn read_dev_desc(&mut self) -> Result<()> {
        let reg: Mmio<Volatile<DeviceDescriptor>> = unsafe {
            Mmio::from_raw(
                self.dev_desc_buf_mem_info
                    .frame_start_virt_addr()?
                    .as_ptr_mut(),
            )
        };
        reg.as_ref().read();

        Ok(())
    }

    pub fn read_conf_descs(&mut self) -> Result<()> {
        let conf_desc_reg: Mmio<Volatile<ConfigurationDescriptor>> = unsafe {
            Mmio::from_raw(
                self.conf_desc_buf_mem_info
                    .frame_start_virt_addr()?
                    .as_ptr_mut(),
            )
        };
        let conf_desc = conf_desc_reg.as_ref().read();

        let mut descs = Vec::new();
        let mut offset = conf_desc.header.length as usize;

        descs.push(Descriptor::Configuration(conf_desc));

        loop {
            let addr = self
                .conf_desc_buf_mem_info
                .frame_start_virt_addr()?
                .offset(offset);

            let desc_header_reg: Mmio<Volatile<DescriptorHeader>> =
                unsafe { Mmio::from_raw(addr.as_ptr_mut()) };
            let desc_header = desc_header_reg.as_ref().read();

            if desc_header.length == 0 {
                break;
            }

            offset += desc_header.length as usize;

            let desc = match desc_header.ty {
                DescriptorType::Device => {
                    let dev_desc_reg: Mmio<Volatile<DeviceDescriptor>> =
                        unsafe { Mmio::from_raw(addr.as_ptr_mut()) };
                    Descriptor::Device(dev_desc_reg.as_ref().read())
                }
                DescriptorType::Configration => {
                    let conf_desc_reg: Mmio<Volatile<ConfigurationDescriptor>> =
                        unsafe { Mmio::from_raw(addr.as_ptr_mut()) };
                    Descriptor::Configuration(conf_desc_reg.as_ref().read())
                }
                DescriptorType::Endpoint => {
                    let endpoint_desc_reg: Mmio<Volatile<EndpointDescriptor>> =
                        unsafe { Mmio::from_raw(addr.as_ptr_mut()) };
                    Descriptor::Endpoint(endpoint_desc_reg.as_ref().read())
                }
                DescriptorType::Interface => {
                    let interface_desc_reg: Mmio<Volatile<InterfaceDescriptor>> =
                        unsafe { Mmio::from_raw(addr.as_ptr_mut()) };
                    Descriptor::Interface(interface_desc_reg.as_ref().read())
                }
                DescriptorType::HumanInterfaceDevice => {
                    let hid_desc_reg: Mmio<Volatile<HumanInterfaceDeviceDescriptor>> =
                        unsafe { Mmio::from_raw(addr.as_ptr_mut()) };
                    let hid_desc = hid_desc_reg.as_ref().read();
                    let num_descs = hid_desc.num_descs as usize;
                    let mut class_desc_headers = Vec::new();

                    for i in 0..num_descs {
                        let addr = addr.offset(size_of::<DescriptorHeader>() * i);
                        let class_desc_header_reg: Mmio<Volatile<DescriptorHeader>> =
                            unsafe { Mmio::from_raw(addr.as_ptr_mut()) };
                        class_desc_headers.push(class_desc_header_reg.as_ref().read());
                    }

                    Descriptor::HumanInterfaceDevice(hid_desc, class_desc_headers)
                }
                other => {
                    let desc_header_reg: Mmio<Volatile<DescriptorHeader>> =
                        unsafe { Mmio::from_raw(addr.as_ptr_mut()) };
                    let desc_header = desc_header_reg.as_ref().read();
                    Descriptor::Unsupported((other, desc_header))
                }
            };
            descs.push(desc);
        }

        self.conf_descs = descs;
        Ok(())
    }

    pub fn get_dev_desc(&self) -> &DeviceDescriptor {
        &self.dev_desc
    }

    pub fn get_conf_descs(&self) -> &Vec<Descriptor> {
        &self.conf_descs
    }

    pub fn request_to_get_desc(
        &mut self,
        desc_type: DescriptorType,
        desc_index: usize,
    ) -> Result<()> {
        let buf_mem_info = match desc_type {
            DescriptorType::Device => self.dev_desc_buf_mem_info,
            DescriptorType::Configration => self.conf_desc_buf_mem_info,
            _ => unimplemented!(),
        };

        let setup_value = match desc_type {
            DescriptorType::Device => 0x100,
            DescriptorType::Configration => (desc_type as u16) << 8 | desc_index as u16,
            _ => unimplemented!(),
        };

        match desc_type {
            DescriptorType::Configration => self.current_conf_index = desc_index,
            _ => (),
        }

        let buf_size = buf_mem_info.frame_size;

        self.ctrl_in(
            RequestType::Standard,
            RequestTypeRecipient::Device,
            SetupRequest::GetDescriptor,
            setup_value,
            0,
            buf_mem_info.frame_size as u16,
            Some((buf_mem_info.frame_start_virt_addr()?, buf_size as u32)),
        )
    }

    pub fn request_to_set_conf(&mut self, conf_value: u8) -> Result<()> {
        self.ctrl_out(
            RequestType::Standard,
            RequestTypeRecipient::Device,
            SetupRequest::SetConfiguration,
            conf_value as u16,
            0,
            0,
            None,
        )
    }

    pub fn get_num_confs(&self) -> usize {
        self.get_dev_desc().num_configs as usize
    }

    pub fn get_interface_descs(&self) -> Vec<&InterfaceDescriptor> {
        self.conf_descs
            .iter()
            .filter(|d| matches!(**d, Descriptor::Interface(_)))
            .map(|d| match d {
                Descriptor::Interface(desc) => desc,
                _ => unreachable!(),
            })
            .collect()
    }

    pub fn get_endpoint_descs(&self) -> Vec<&EndpointDescriptor> {
        self.conf_descs
            .iter()
            .filter(|d| matches!(**d, Descriptor::Endpoint(_)))
            .map(|d| match d {
                Descriptor::Endpoint(desc) => desc,
                _ => unreachable!(),
            })
            .collect()
    }

    pub fn configure_endpoint(&mut self, endpoint_type: EndpointType) -> Result<()> {
        let mut configured_endpoint_dci = self.configured_endpoint_dci.clone();

        let device_context = device::usb::xhc::read_device_context(self.slot_id)?.unwrap();
        let mut input_context =
            device::usb::xhc::read_port_input_context_by_slot_id(self.slot_id)?.unwrap();
        input_context.device_context.slot_context = device_context.slot_context;
        let mut input_ctrl_context = InputControlContext::default();
        input_ctrl_context.set_add_context_flag(0, true).unwrap();

        let mut ring_buf_buf = Vec::new();
        for endpoint_desc in self.get_endpoint_descs() {
            let endpoint_addr = endpoint_desc.endpoint_addr;
            let dci = endpoint_desc.dci();

            let mut endpoint_context = EndpointContext::default();
            let desc_endpoint_type = EndpointType::new(endpoint_addr, endpoint_desc.bitmap_attrs);
            if desc_endpoint_type != endpoint_type {
                continue;
            }

            let mut transfer_ring_buf = RingBuffer::new(RingBufferType::TransferRing, true)?;
            transfer_ring_buf.set_link_trb()?;

            endpoint_context.set_endpoint_type(endpoint_type);
            endpoint_context.set_max_packet_size(self.max_packet_size);
            endpoint_context.set_max_endpoint_service_interval_payload_low(self.max_packet_size);
            endpoint_context.set_max_burst_size(0);
            endpoint_context.set_dequeue_cycle_state(true); // initial cycle state of transfer ring buffer
            endpoint_context.set_tr_dequeue_ptr(transfer_ring_buf.buf_ptr() as u64);
            endpoint_context.set_interval(endpoint_desc.interval - 1);
            endpoint_context.set_max_primary_streams(0);
            endpoint_context.set_mult(0);
            endpoint_context.set_error_cnt(3);
            endpoint_context.set_average_trb_len(1);

            input_context.device_context.endpoint_contexts[dci - 1] = endpoint_context;
            input_ctrl_context.set_add_context_flag(dci, true).unwrap();

            ring_buf_buf.push((dci, transfer_ring_buf));
            configured_endpoint_dci.push(dci);
        }

        for (dci, ring_buf) in ring_buf_buf {
            self.transfer_ring_bufs[dci] = Some(ring_buf);
        }

        input_context.input_ctrl_context = input_ctrl_context;
        device::usb::xhc::write_port_input_context_by_slot_id(self.slot_id, input_context)?;

        self.configured_endpoint_dci = configured_endpoint_dci;

        let trb = device::usb::xhc::generate_config_endpoint_trb(self.slot_id)?;
        device::usb::xhc::push_cmd_ring(trb)
    }

    pub fn configure_endpoint_transfer_ring(&mut self) -> Result<()> {
        for endpoint_id in &self.configured_endpoint_dci {
            if let Some(ring_buf) = self.transfer_ring_bufs[*endpoint_id].as_mut() {
                let mut trb = TransferRequestBlock::default();
                trb.set_trb_type(TransferRequestBlockType::Normal);
                trb.param = 0;
                trb.status = 8; // TRB Transfer Length
                trb.set_other_flags(0x12); // IOC, ISP bit

                ring_buf.fill_and_alloc_buf(trb)?;
                device::usb::xhc::ring_doorbell(self.slot_id, *endpoint_id as u8)?;
            }
        }

        Ok(())
    }

    pub fn request_to_set_interface(&mut self, interface_desc: InterfaceDescriptor) -> Result<()> {
        self.ctrl_out(
            RequestType::Standard,
            RequestTypeRecipient::Interface,
            SetupRequest::SetInterface,
            interface_desc.alternate_setting as u16,
            interface_desc.interface_num as u16,
            0,
            None,
        )
    }

    pub fn request_to_set_protocol(
        &mut self,
        interface_desc: InterfaceDescriptor,
        protocol: u8,
    ) -> Result<()> {
        self.ctrl_out(
            RequestType::Class,
            RequestTypeRecipient::Interface,
            SetupRequest::SET_PROTOCOL,
            protocol as u16,
            interface_desc.interface_num as u16,
            0,
            None,
        )
    }

    pub fn update(&mut self, endpoint_id: usize, transfer_event_trb: TransferRequestBlock) {
        if let Some(ring_buf) = self.transfer_ring_bufs[endpoint_id].as_mut() {
            //ring_buf.debug();

            let data_trb_ptr = transfer_event_trb.param as *const TransferRequestBlock;
            let data_trb = unsafe { data_trb_ptr.read() };
            let data_ptr = data_trb.param as *const InputData;
            let _ = unsafe { data_ptr.read() };

            ring_buf.enqueue().unwrap();
        }
    }

    fn ctrl_out(
        &mut self,
        req_type: RequestType,
        req_type_recipient: RequestTypeRecipient,
        setup_req: SetupRequest,
        setup_value: u16,
        setup_index: u16,
        setup_length: u16,
        data: Option<(VirtualAddress, u32)>, // buf addr, buf size
    ) -> Result<()> {
        if (setup_length > 0 && data == None) || (setup_length == 0 && data != None) {
            return Err(UsbDeviceError::InvalidRequestError.into());
        }

        let mut setup_stage_trb = TransferRequestBlock::default();
        setup_stage_trb.set_trb_type(TransferRequestBlockType::SetupStage);

        let mut setup_req_type = SetupRequestType::default();
        setup_req_type.set_direction(RequestTypeDirection::Out);
        setup_req_type.set_ty(req_type);
        setup_req_type.set_recipient(req_type_recipient);

        setup_stage_trb.set_setup_request_type(setup_req_type);
        setup_stage_trb.set_setup_request(setup_req);
        setup_stage_trb.set_setup_index(setup_index);
        setup_stage_trb.set_setup_value(setup_value);
        setup_stage_trb.set_setup_length(setup_length);
        setup_stage_trb.status = 8; // TRB transfer length
        setup_stage_trb.set_other_flags(1 << 5); // IDT bit

        let data_stage_trb = match data {
            Some((buf_phys_addr, buf_size)) => {
                setup_stage_trb.set_transfer_type(TransferType::OutDataStage);
                let mut trb = TransferRequestBlock::default();
                trb.set_trb_type(TransferRequestBlockType::DataStage);
                trb.param = buf_phys_addr.get();
                trb.status = buf_size;
                trb.set_other_flags(1 << 4); // IOC bit
                trb.ctrl_regs = 0; // DIR bit
                Some(trb)
            }
            None => {
                setup_stage_trb.set_transfer_type(TransferType::NoDataStage);
                setup_stage_trb.set_other_flags(setup_stage_trb.other_flags() | 1 << 4); // IOC bit
                None
            }
        };

        let mut status_stage_trb = TransferRequestBlock::default();
        status_stage_trb.set_trb_type(TransferRequestBlockType::StatusStage);
        status_stage_trb.ctrl_regs = 1; // DIR bit

        self.send_to_dcp_transfer_ring(setup_stage_trb, data_stage_trb, Some(status_stage_trb))
    }

    fn ctrl_in(
        &mut self,
        req_type: RequestType,
        req_type_recipient: RequestTypeRecipient,
        setup_req: SetupRequest,
        setup_value: u16,
        setup_index: u16,
        setup_length: u16,
        data: Option<(VirtualAddress, u32)>, // buf addr, buf size
    ) -> Result<()> {
        if (setup_length > 0 && data == None) || (setup_length == 0 && data != None) {
            return Err(UsbDeviceError::InvalidRequestError.into());
        }

        let mut setup_stage_trb = TransferRequestBlock::default();
        setup_stage_trb.set_trb_type(TransferRequestBlockType::SetupStage);

        let mut setup_req_type = SetupRequestType::default();
        setup_req_type.set_direction(RequestTypeDirection::In);
        setup_req_type.set_ty(req_type);
        setup_req_type.set_recipient(req_type_recipient);

        setup_stage_trb.set_setup_request_type(setup_req_type);
        setup_stage_trb.set_setup_request(setup_req);
        setup_stage_trb.set_setup_index(setup_index);
        setup_stage_trb.set_setup_value(setup_value);
        setup_stage_trb.set_setup_length(setup_length);
        setup_stage_trb.status = 8; // TRB transfer length
        setup_stage_trb.set_other_flags(1 << 5); // IDT bit

        let data_stage_trb = match data {
            Some((buf_phys_addr, buf_size)) => {
                setup_stage_trb.set_transfer_type(TransferType::InDataStage);
                let mut trb = TransferRequestBlock::default();
                trb.set_trb_type(TransferRequestBlockType::DataStage);
                trb.param = buf_phys_addr.get();
                trb.status = buf_size;
                trb.set_other_flags(1 << 4); // IOC bit
                trb.ctrl_regs = 1; // DIR bit
                Some(trb)
            }
            None => {
                setup_stage_trb.set_transfer_type(TransferType::NoDataStage);
                setup_stage_trb.set_other_flags(setup_stage_trb.other_flags() | 1 << 4); // IOC bit
                None
            }
        };

        let mut status_stage_trb = TransferRequestBlock::default();
        status_stage_trb.set_trb_type(TransferRequestBlockType::StatusStage);

        let ctrl_regs = match data_stage_trb {
            Some(_) => 0,
            None => 1,
        };

        status_stage_trb.ctrl_regs = ctrl_regs; // DIR bit

        self.send_to_dcp_transfer_ring(setup_stage_trb, data_stage_trb, Some(status_stage_trb))
    }

    fn send_to_dcp_transfer_ring(
        &mut self,
        setup_stage_trb: TransferRequestBlock,
        data_stage_trb: Option<TransferRequestBlock>,
        status_stage_trb: Option<TransferRequestBlock>,
    ) -> Result<()> {
        if setup_stage_trb.trb_type() != TransferRequestBlockType::SetupStage
            || (data_stage_trb.is_some()
                && data_stage_trb.unwrap().trb_type() != TransferRequestBlockType::DataStage)
            || (status_stage_trb.is_some()
                && status_stage_trb.unwrap().trb_type() != TransferRequestBlockType::StatusStage)
        {
            return Err(UsbDeviceError::InvalidTransferRequestBlockTypeError.into());
        }

        let dcp_transfer_ring = self.transfer_ring_bufs[1].as_mut().unwrap();

        dcp_transfer_ring.push(setup_stage_trb)?;

        if let Some(trb) = data_stage_trb {
            dcp_transfer_ring.push(trb)?;
        }

        if let Some(trb) = status_stage_trb {
            dcp_transfer_ring.push(trb)?;
        }

        device::usb::xhc::ring_doorbell(self.slot_id, DEFAULT_CTRL_PIPE_ID)
    }
}
