use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{
    arch::{mmio::Mmio, pin::IntoPinnedMutableSlice},
    debug,
    device::{
        self,
        pci_bus::conf_space::BaseAddress,
        usb_bus::{UsbDevice, UsbDeviceAttachInfo},
        xhc::{context::*, desc::*, register::*, trb::*},
    },
    error::{Error, Result},
    fs::vfs,
    info,
    mem::{bitmap, paging::PAGE_SIZE},
    trace,
    util::mutex::Mutex,
};
use alloc::{
    boxed::Box,
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use core::{cmp::max, pin::Pin, slice};

pub mod context;
pub mod desc;
pub mod register;
pub mod trb;

static mut XHC_DRIVER: Mutex<XhcDriver> = Mutex::new(XhcDriver::new());

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum XhcDriverError {
    InvalidRegisterAddress,
    RegisterNotInitialized,
    HostControllerIsNotHalted,
    EventRingNotInitialized,
    DeviceContextBaseAddressArrayNotInitialized,
    CommandRingNotInitialized,
    PortScNotInitialized,
    PortNotConnected(usize),
}

pub trait XhcRequestFunction {
    fn set_config(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        config_value: u8,
    ) -> Result<()>;
    fn set_interface(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        interface_num: u8,
        alt_setting: u8,
    ) -> Result<()>;
    fn set_protocol(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        interface_num: u8,
        protocol: u8,
    ) -> Result<()>;
    fn hid_report(&mut self, slot: u8, ctrl_ep_ring: &mut CommandRing) -> Result<Vec<u8>>;
}

struct XhcDriver {
    device_driver_info: DeviceDriverInfo,
    pci_device_bdf: Option<(usize, usize, usize)>,
    cap_reg: Option<Mmio<CapabilityRegisters>>,
    ope_reg: Option<Mmio<OperationalRegisters>>,
    rt_reg: Option<Mmio<RuntimeRegisters>>,
    dcbaa: Option<DeviceContextBaseAddressArray>,
    primary_event_ring: Option<EventRing>,
    cmd_ring: Option<CommandRing>,
    portsc: Option<PortSc>,
    doorbell_regs: Vec<Rc<Doorbell>>,
}

impl XhcDriver {
    const fn new() -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("xhc"),
            pci_device_bdf: None,
            cap_reg: None,
            ope_reg: None,
            rt_reg: None,
            dcbaa: None,
            primary_event_ring: None,
            cmd_ring: None,
            portsc: None,
            doorbell_regs: Vec::new(),
        }
    }

    fn cap_reg(&mut self) -> Result<&mut Mmio<CapabilityRegisters>> {
        self.cap_reg
            .as_mut()
            .ok_or(XhcDriverError::RegisterNotInitialized.into())
    }

    fn ope_reg(&mut self) -> Result<&mut Mmio<OperationalRegisters>> {
        self.ope_reg
            .as_mut()
            .ok_or(XhcDriverError::RegisterNotInitialized.into())
    }

    fn rt_reg(&mut self) -> Result<&mut Mmio<RuntimeRegisters>> {
        self.rt_reg
            .as_mut()
            .ok_or(XhcDriverError::RegisterNotInitialized.into())
    }

    fn dcbaa(&mut self) -> Result<&mut DeviceContextBaseAddressArray> {
        self.dcbaa
            .as_mut()
            .ok_or(XhcDriverError::DeviceContextBaseAddressArrayNotInitialized.into())
    }

    fn primary_event_ring(&mut self) -> Result<&mut EventRing> {
        self.primary_event_ring
            .as_mut()
            .ok_or(XhcDriverError::EventRingNotInitialized.into())
    }

    fn cmd_ring(&mut self) -> Result<&mut CommandRing> {
        self.cmd_ring
            .as_mut()
            .ok_or(XhcDriverError::CommandRingNotInitialized.into())
    }

    fn portsc(&self) -> Result<&PortSc> {
        self.portsc
            .as_ref()
            .ok_or(XhcDriverError::PortScNotInitialized.into())
    }

    fn doorbell(&self, index: usize) -> Result<&Rc<Doorbell>> {
        self.doorbell_regs
            .get(index)
            .ok_or(Error::IndexOutOfBoundsError(index))
    }

    fn notify(&self) -> Result<()> {
        self.doorbell(0)?.notify(0, 0);
        Ok(())
    }

    fn notify_ep(&self, slot: u8, dci: usize) -> Result<()> {
        let db = self.doorbell(slot as usize)?;
        db.notify(dci as u8, 0);
        Ok(())
    }

    fn send_cmd(&mut self, cmd: GenericTrbEntry) -> Result<GenericTrbEntry> {
        self.cmd_ring()?.push(cmd)?;
        self.notify()?;
        loop {
            if let Some(trb) = self.primary_event_ring()?.pop()? {
                if trb.trb_type() == TrbType::CommandCompletionEvent as u32 {
                    return Ok(trb);
                } else {
                    trace!("Invalid TRB type: 0x{:x}", trb.trb_type());
                }
            }
        }
    }

    fn reset(&mut self) -> Result<()> {
        let driver_name = self.device_driver_info.name;

        // stop controller
        if !self.ope_reg()?.as_ref().usb_status.hchalted() {
            return Err(XhcDriverError::HostControllerIsNotHalted.into());
        }

        // reset controller
        self.ope_reg()?
            .as_mut()
            .usb_cmd
            .set_host_controller_reset(true);

        loop {
            debug!("{}: Waiting xHC...", driver_name);
            if !self.ope_reg()?.as_ref().usb_cmd.host_controller_reset() {
                break;
            }
        }
        debug!("{}: xHC reset complete", driver_name);

        Ok(())
    }

    fn set_max_dev_slots(&mut self) -> Result<()> {
        let driver_name = self.device_driver_info.name;

        let num_of_ports = self.cap_reg()?.as_ref().num_of_ports();
        let num_of_slots = self.cap_reg()?.as_ref().num_of_device_slots();
        self.ope_reg()?
            .as_mut()
            .set_max_device_slots_enabled(num_of_slots as u8);
        debug!("{}: Number of ports: {}", driver_name, num_of_ports);

        Ok(())
    }

    fn init_scratchpad_bufs(&mut self) -> Result<ScratchpadBuffers> {
        let driver_name = self.device_driver_info.name;

        let num_scratchpad_bufs = max(self.cap_reg()?.as_ref().num_scratchpad_bufs(), 1);
        debug!(
            "{}: Number of scratchpad buffers: {}",
            driver_name, num_scratchpad_bufs
        );

        // buffer table
        // non-deallocate memory
        let mem_frame_info = bitmap::alloc_mem_frame(
            (size_of::<usize>() * num_scratchpad_bufs).div_ceil(PAGE_SIZE),
        )?;
        let table = unsafe {
            slice::from_raw_parts(
                mem_frame_info.frame_start_virt_addr()?.as_ptr_mut() as *mut *const u8,
                num_scratchpad_bufs,
            )
        };
        let mut table: Pin<Box<[*const u8]>> = Pin::new(Box::from(table));

        // buffer
        let mut bufs = Vec::new();
        for sb in table.iter_mut() {
            // non-deallocate memory
            let sb_frame_info = bitmap::alloc_mem_frame(1)?;
            let buf_ptr = sb_frame_info.frame_start_virt_addr()?.as_ptr();
            let buf = unsafe { slice::from_raw_parts(buf_ptr as *const u8, PAGE_SIZE) };
            let buf: Pin<Box<[u8]>> = Pin::new(Box::from(buf));
            *sb = buf.as_ref().as_ptr();
            bufs.push(buf);
        }
        let scratchpad_bufs = ScratchpadBuffers { table, bufs };
        debug!("{}: Scratchpad buffers initialized", driver_name);
        Ok(scratchpad_bufs)
    }

    fn init_dev_ctx(&mut self, scratchpad_bufs: ScratchpadBuffers) -> Result<()> {
        let driver_name = self.device_driver_info.name;

        // initialize device context
        let dcbaa = DeviceContextBaseAddressArray::new(scratchpad_bufs);
        self.ope_reg()?
            .as_mut()
            .dcbaa_ptr
            .write(dcbaa.inner_mut_ptr());
        self.dcbaa = Some(dcbaa);
        debug!(
            "{}: Device context base address array initialized",
            driver_name
        );

        Ok(())
    }

    fn init_primary_event_ring(&mut self) -> Result<()> {
        let driver_name = self.device_driver_info.name;

        self.primary_event_ring = Some(EventRing::new()?);
        let event_ring = self.primary_event_ring.as_mut().unwrap();
        let rt_reg = unsafe { self.rt_reg.as_mut().unwrap().get_unchecked_mut() };
        rt_reg.init_int_reg_set(0, event_ring)?;
        debug!("{}: Primary event ring initialized", driver_name);

        Ok(())
    }

    fn init_cmd_ring(&mut self) -> Result<()> {
        let driver_name = self.device_driver_info.name;

        self.cmd_ring = Some(CommandRing::default());
        let cmd_ring = self.cmd_ring.as_mut().unwrap();
        let ope_reg = unsafe { self.ope_reg.as_mut().unwrap().get_unchecked_mut() };
        ope_reg.set_cmd_ring_ctrl(cmd_ring);
        debug!("{}: Command ring initialized", driver_name);

        Ok(())
    }

    fn init_port(&mut self, port: usize) -> Result<u8> {
        let driver_name = self.device_driver_info.name;

        let e = self
            .portsc()?
            .get(port)
            .ok_or(Error::IndexOutOfBoundsError(port))?;
        if !e.ccs() {
            return Err(XhcDriverError::PortNotConnected(port).into());
        }
        e.reset_port();
        assert!(e.is_enabled());

        let trb = self.send_cmd(GenericTrbEntry::trb_enable_slot_cmd())?;
        let slot = trb.slot_id();

        debug!(
            "{}: Port {} is connected to slot {}",
            driver_name, port, slot
        );
        Ok(slot)
    }

    fn set_output_context_for_slot(
        &mut self,
        slot: u8,
        output_context: Pin<Box<OutputContext>>,
    ) -> Result<()> {
        self.dcbaa()?.set_output_context(slot, output_context)?;
        Ok(())
    }

    fn address_device(&mut self, port: usize, slot: u8) -> Result<CommandRing> {
        let driver_name = self.device_driver_info.name;

        let output_context = Box::pin(OutputContext::default());
        self.set_output_context_for_slot(slot, output_context)?;
        let mut input_ctrl_context = InputControlContext::default();
        input_ctrl_context.add_context(0)?;
        input_ctrl_context.add_context(1)?;
        let mut input_context = Box::pin(InputContext::default());
        input_context
            .as_mut()
            .set_input_ctrl_context(input_ctrl_context);
        input_context.as_mut().set_root_hub_port_num(port)?;
        input_context.as_mut().set_last_valid_dci(1)?;

        let portsc_e = self
            .portsc()?
            .get(port)
            .ok_or(Error::IndexOutOfBoundsError(port))?;
        let port_speed = portsc_e.port_speed();
        trace!("{:?}", port_speed);
        input_context.as_mut().set_port_speed(port_speed)?;
        let ctrl_ep_ring = CommandRing::default();
        input_context.as_mut().set_ep_context(
            1,
            EndpointContext::new_ctrl_endpoint(
                portsc_e.max_packet_size()?,
                ctrl_ep_ring.ring_phys_addr(),
            )?,
        );

        let cmd = GenericTrbEntry::trb_cmd_address_device(input_context.as_ref(), slot);
        self.send_cmd(cmd)?.cmd_result_ok()?;

        debug!(
            "{}: Addressed device on port {} with slot {}",
            driver_name, port, slot
        );
        Ok(ctrl_ep_ring)
    }

    fn request_desc<T: Sized>(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        desc_type: UsbDescriptorType,
        desc_index: u8,
        lang_id: u16,
        buf: Pin<&mut [T]>,
    ) -> Result<()> {
        ctrl_ep_ring.push(
            SetupStageTrb::new(
                SetupStageTrb::REQ_TYPE_DIR_DEV_TO_HOST,
                SetupStageTrb::REQ_GET_DESC,
                (desc_type as u16) << 8 | (desc_index as u16),
                lang_id,
                (buf.len() * size_of::<T>()) as u16,
            )
            .into(),
        )?;
        ctrl_ep_ring.push(DataStageTrb::new_in(buf).into())?;
        ctrl_ep_ring.push(StatusStageTrb::new_out().into())?;
        self.notify_ep(slot, 1)?;
        loop {
            if let Some(trb) = self.primary_event_ring()?.pop()? {
                if trb.transfer_result_ok().is_ok() {
                    break;
                }
            }
        }

        Ok(())
    }

    fn request_dev_desc(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
    ) -> Result<UsbDeviceDescriptor> {
        let mut desc = Box::pin(UsbDeviceDescriptor::default());
        self.request_desc(
            slot,
            ctrl_ep_ring,
            UsbDescriptorType::Device,
            0,
            0,
            desc.as_mut().as_mut_slice(),
        )?;
        Ok(*desc)
    }

    fn request_string_desc(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        lang_id: u16,
        index: u8,
    ) -> Result<String> {
        let buf = vec![0; 128];
        let mut buf = Box::into_pin(buf.into_boxed_slice());
        self.request_desc(
            slot,
            ctrl_ep_ring,
            UsbDescriptorType::String,
            index,
            lang_id,
            buf.as_mut(),
        )?;
        let s = String::from_utf8_lossy(&buf[2..])
            .to_string()
            .replace("\0", "");
        Ok(s)
    }

    fn request_string_desc_zero(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
    ) -> Result<Vec<u16>> {
        let buf = vec![0; 8];
        let mut buf = Box::into_pin(buf.into_boxed_slice());
        self.request_desc(
            slot,
            ctrl_ep_ring,
            UsbDescriptorType::String,
            0,
            0,
            buf.as_mut(),
        )?;
        Ok(buf.as_ref().get_ref().to_vec())
    }

    fn request_conf_desc_and_rest(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
    ) -> Result<Vec<UsbDescriptor>> {
        let mut conf_desc = Box::pin(ConfigDescriptor::default());
        self.request_desc(
            slot,
            ctrl_ep_ring,
            UsbDescriptorType::Config,
            0,
            0,
            conf_desc.as_mut().as_mut_slice(),
        )?;

        let buf = vec![0; conf_desc.total_len()];
        let mut buf = Box::into_pin(buf.into_boxed_slice());
        self.request_desc(
            slot,
            ctrl_ep_ring,
            UsbDescriptorType::Config,
            0,
            0,
            buf.as_mut(),
        )?;

        let iter = DescriptorIterator::new(&buf);
        let descs: Vec<UsbDescriptor> = iter.collect();
        Ok(descs)
    }

    fn request_set_protocol(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        interface_num: u8,
        protocol: u8,
    ) -> Result<()> {
        ctrl_ep_ring.push(
            SetupStageTrb::new(
                SetupStageTrb::REQ_TYPE_TO_INTERFACE,
                SetupStageTrb::REQ_SET_PROTOCOL,
                protocol as u16,
                interface_num as u16,
                0,
            )
            .into(),
        )?;
        ctrl_ep_ring.push(StatusStageTrb::new_in().into())?;
        self.notify_ep(slot, 1)?;
        loop {
            if let Some(trb) = self.primary_event_ring()?.pop()? {
                if trb.transfer_result_ok().is_ok() {
                    break;
                }
            }
        }

        Ok(())
    }

    fn request_set_interface(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        interface_num: u8,
        alt_setting: u8,
    ) -> Result<()> {
        ctrl_ep_ring.push(
            SetupStageTrb::new(
                SetupStageTrb::REQ_TYPE_TO_INTERFACE,
                SetupStageTrb::REQ_SET_INTERFACE,
                alt_setting as u16,
                interface_num as u16,
                0,
            )
            .into(),
        )?;
        ctrl_ep_ring.push(StatusStageTrb::new_in().into())?;
        self.notify_ep(slot, 1)?;
        loop {
            if let Some(trb) = self.primary_event_ring()?.pop()? {
                if trb.transfer_result_ok().is_ok() {
                    break;
                }
            }
        }

        Ok(())
    }

    fn request_set_config(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        config_value: u8,
    ) -> Result<()> {
        ctrl_ep_ring.push(
            SetupStageTrb::new(0, SetupStageTrb::REQ_SET_CONF, config_value as u16, 0, 0).into(),
        )?;
        ctrl_ep_ring.push(StatusStageTrb::new_in().into())?;
        self.notify_ep(slot, 1)?;
        loop {
            if let Some(trb) = self.primary_event_ring()?.pop()? {
                if trb.transfer_result_ok().is_ok() {
                    break;
                }
            }
        }

        Ok(())
    }

    fn request_report_bytes(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        buf: Pin<&mut [u8]>,
    ) -> Result<()> {
        ctrl_ep_ring.push(
            SetupStageTrb::new(
                SetupStageTrb::REQ_TYPE_DIR_DEV_TO_HOST
                    | SetupStageTrb::REQ_TYPE_TYPE_CLASS
                    | SetupStageTrb::REQ_TYPE_TO_INTERFACE,
                SetupStageTrb::REQ_GET_REPORT,
                0x0200,
                0,
                buf.len() as u16,
            )
            .into(),
        )?;
        ctrl_ep_ring.push(DataStageTrb::new_in(buf).into())?;
        ctrl_ep_ring.push(StatusStageTrb::new_out().into())?;
        self.notify_ep(slot, 1)?;
        loop {
            if let Some(trb) = self.primary_event_ring()?.pop()? {
                if trb.transfer_result_ok().is_ok() {
                    break;
                }
            }
        }

        Ok(())
    }

    fn request_hid_report(&mut self, slot: u8, ctrl_ep_ring: &mut CommandRing) -> Result<Vec<u8>> {
        let buf = [0u8; 8];
        let mut buf = Box::into_pin(Box::new(buf));
        self.request_report_bytes(slot, ctrl_ep_ring, buf.as_mut())?;
        Ok(buf.to_vec())
    }

    fn start(&mut self) -> Result<()> {
        let driver_name = self.device_driver_info.name;
        self.ope_reg()?.as_mut().usb_cmd.set_run_stop(true);

        loop {
            debug!("{}: Waiting xHC...", driver_name);
            if !self.ope_reg()?.as_ref().usb_status.hchalted() {
                break;
            }
        }
        debug!("{}: xHC started", driver_name);

        // initialize ports
        for port in self.portsc()?.port_range() {
            if let Some(e) = self.portsc()?.get(port) {
                // skip disconnected devices
                if !e.ccs() {
                    continue;
                }

                let slot = self.init_port(port)?;
                let mut ctrl_ep_ring = self.address_device(port, slot)?;
                let dev_desc = self.request_dev_desc(slot, &mut ctrl_ep_ring)?;

                let mut vendor = None;
                let mut product = None;
                let mut serial = None;
                if let Ok(e) = self.request_string_desc_zero(slot, &mut ctrl_ep_ring) {
                    let lang_id = e[1];
                    if dev_desc.manufacturer_index != 0 {
                        vendor = Some(self.request_string_desc(
                            slot,
                            &mut ctrl_ep_ring,
                            lang_id,
                            dev_desc.manufacturer_index,
                        )?);
                    }

                    if dev_desc.product_index != 0 {
                        product = Some(self.request_string_desc(
                            slot,
                            &mut ctrl_ep_ring,
                            lang_id,
                            dev_desc.product_index,
                        )?);
                    }

                    if dev_desc.serial_index != 0 {
                        serial = Some(self.request_string_desc(
                            slot,
                            &mut ctrl_ep_ring,
                            lang_id,
                            dev_desc.serial_index,
                        )?);
                    }
                }

                let descs = self.request_conf_desc_and_rest(slot, &mut ctrl_ep_ring)?;
                debug!("{}: Port {} initialized", driver_name, port);

                // attach usb device
                let attach_info = UsbDeviceAttachInfo::new_xhci(
                    port,
                    slot,
                    vendor,
                    product,
                    serial,
                    dev_desc,
                    descs,
                    Box::new(ctrl_ep_ring),
                );

                let usb_device = UsbDevice::new(attach_info);
                device::usb_bus::attach_usb_device(usb_device)?;
            }
        }

        Ok(())
    }
}

impl XhcRequestFunction for XhcDriver {
    fn set_config(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        config_value: u8,
    ) -> Result<()> {
        self.request_set_config(slot, ctrl_ep_ring, config_value)
    }

    fn set_interface(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        interface_num: u8,
        alt_setting: u8,
    ) -> Result<()> {
        self.request_set_interface(slot, ctrl_ep_ring, interface_num, alt_setting)
    }

    fn set_protocol(
        &mut self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        interface_num: u8,
        protocol: u8,
    ) -> Result<()> {
        self.request_set_protocol(slot, ctrl_ep_ring, interface_num, protocol)
    }

    fn hid_report(&mut self, slot: u8, ctrl_ep_ring: &mut CommandRing) -> Result<Vec<u8>> {
        self.request_hid_report(slot, ctrl_ep_ring)
    }
}

impl DeviceDriverFunction for XhcDriver {
    type AttachInput = ();
    type PollNormalOutput = ();
    type PollInterruptOutput = ();

    fn get_device_driver_info(&self) -> Result<DeviceDriverInfo> {
        Ok(self.device_driver_info.clone())
    }

    fn probe(&mut self) -> Result<()> {
        device::pci_bus::find_devices(0x0c, 0x03, 0x30, |d| {
            self.pci_device_bdf = Some(d.bdf());
            Ok(())
        })?;

        Ok(())
    }

    fn attach(&mut self, _arg: Self::AttachInput) -> Result<()> {
        if self.pci_device_bdf.is_none() {
            return Err(Error::Failed("Device driver is not probed"));
        }

        let driver_name = self.device_driver_info.name;
        let (bus, device, func) = self.pci_device_bdf.unwrap();
        device::pci_bus::configure_device(bus, device, func, |d| {
            // read base address registers
            let conf_space = d.read_conf_space_non_bridge_field()?;
            let bars = conf_space.get_bars()?;
            if bars.len() == 0 {
                return Err(XhcDriverError::InvalidRegisterAddress.into());
            }

            let cap_reg_virt_addr = match bars[0].1 {
                BaseAddress::MemoryAddress32BitSpace(addr, _) => addr.get_virt_addr()?,
                BaseAddress::MemoryAddress64BitSpace(addr, _) => addr.get_virt_addr()?,
                _ => return Err(XhcDriverError::InvalidRegisterAddress.into()),
            };
            let cap_reg: Mmio<CapabilityRegisters> =
                unsafe { Mmio::from_raw(cap_reg_virt_addr.as_ptr_mut()) };
            let ope_reg_offset = cap_reg.as_ref().cap_reg_len();
            let rt_reg_offset = cap_reg.as_ref().rts_offset();

            self.cap_reg = Some(cap_reg);

            let ope_reg =
                unsafe { Mmio::from_raw(cap_reg_virt_addr.offset(ope_reg_offset).as_ptr_mut()) };
            self.ope_reg = Some(ope_reg);

            let rt_reg =
                unsafe { Mmio::from_raw(cap_reg_virt_addr.offset(rt_reg_offset).as_ptr_mut()) };
            self.rt_reg = Some(rt_reg);

            self.portsc = Some(PortSc::new(&cap_reg_virt_addr, self.cap_reg()?.as_ref()));

            let mut doorbell_regs = Vec::new();
            let num_of_slots = self.cap_reg()?.as_ref().num_of_ports();
            for i in 0..=num_of_slots {
                let ptr: *mut u32 = cap_reg_virt_addr
                    .offset(self.cap_reg()?.as_ref().db_offset() + i * 4)
                    .as_ptr_mut();
                doorbell_regs.push(Rc::new(Doorbell::new(ptr)));
            }
            self.doorbell_regs = doorbell_regs;

            self.reset()?;
            self.set_max_dev_slots()?;
            let scratchpad_bufs = self.init_scratchpad_bufs()?;
            self.init_dev_ctx(scratchpad_bufs)?;
            self.init_primary_event_ring()?;
            self.init_cmd_ring()?;
            self.start()?;

            Ok(())
        })?;

        let dev_desc = vfs::DeviceFileDescriptor {
            get_device_driver_info,
            open,
            close,
            read,
            write,
        };
        vfs::add_dev_file(dev_desc, driver_name)?;
        self.device_driver_info.attached = true;
        Ok(())
    }

    fn poll_normal(&mut self) -> Result<Self::PollNormalOutput> {
        if !self.device_driver_info.attached {
            return Err(Error::Failed("Device driver is not attached"));
        }

        let driver_name = self.device_driver_info.name;

        if let Some(trb) = self.primary_event_ring()?.pop()? {
            debug!("{}: Processed TRB: 0x{:x}", driver_name, trb.trb_type());
        }

        Ok(())
    }

    fn poll_int(&mut self) -> Result<Self::PollInterruptOutput> {
        unimplemented!()
    }

    fn open(&mut self) -> Result<()> {
        unimplemented!()
    }

    fn close(&mut self) -> Result<()> {
        unimplemented!()
    }

    fn read(&mut self) -> Result<Vec<u8>> {
        unimplemented!()
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        unimplemented!()
    }
}

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    let driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.probe()?;
    driver.attach(())?;
    info!("{}: Attached!", driver.get_device_driver_info()?.name);
    Ok(())
}

pub fn open() -> Result<()> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.write(data)
}

pub fn poll_normal() -> Result<()> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.poll_normal()
}

pub fn request<R, F: FnOnce(&mut dyn XhcRequestFunction) -> R>(f: F) -> R {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }.unwrap();
    f(&mut *driver)
}
