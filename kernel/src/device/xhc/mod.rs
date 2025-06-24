use core::{cmp::max, pin::Pin, slice};
use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{
    arch::mmio::Mmio, device::{self, pci_bus::conf_space::BaseAddress, xhc::register::*}, error::{Error, Result}, fs::vfs, info, mem::{bitmap, paging::PAGE_SIZE}, trace, util::mutex::Mutex
};
use alloc::{boxed::Box, vec::Vec};

pub mod register;
pub mod context;
pub mod trb;

static mut XHC_DRIVER: Mutex<XhcDriver> = Mutex::new(XhcDriver::new());

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum XhcDriverError {
    InvalidRegisterAddress,
    RegisterNotInitialized,
    HostControllerIsNotHalted,
    EventRingNotInitialized,
    CommandRingNotInitialized,
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
            .ok_or(XhcDriverError::RegisterNotInitialized.into())
    }

    fn reset(&mut self) -> Result<()> {
        let driver_name = self.device_driver_info.name;

        // stop controller
        if !self.ope_reg()?.as_ref().usb_status.hchalted() {
            return Err(XhcDriverError::HostControllerIsNotHalted.into());
        }

        // reset controller
        self.ope_reg()?.as_mut().usb_cmd.set_host_controller_reset(true);

        loop {
            trace!("{}: Waiting xHC...", driver_name);
            if !self.ope_reg()?.as_ref().usb_cmd.host_controller_reset() {
                break;
            }
        }
        trace!("{}: xHC reset complete", driver_name);

        Ok(())
    }

    fn set_max_dev_slots(&mut self) -> Result<()> {
        let driver_name = self.device_driver_info.name;

        let num_of_ports = self.cap_reg()?.as_ref().num_of_ports();
        let num_of_slots = self.cap_reg()?.as_ref().num_of_device_slots();
        self.ope_reg()?.as_mut().set_max_device_slots_enabled(num_of_slots as u8);
        trace!("{}: Number of ports: {}", driver_name, num_of_ports);

        Ok(())
    }

    fn init_scratchpad_bufs(&mut self) -> Result<ScratchpadBuffers> {
        let driver_name = self.device_driver_info.name;

        let num_scratchpad_bufs = max(self.cap_reg()?.as_ref().num_scratchpad_bufs(), 1);
        trace!("{}: Number of scratchpad buffers: {}", driver_name, num_scratchpad_bufs);

        // buffer table
        // non-deallocate memory
        let mem_frame_info = bitmap::alloc_mem_frame((size_of::<usize>() * num_scratchpad_bufs).div_ceil(PAGE_SIZE))?;
        let table = unsafe {
            slice::from_raw_parts(mem_frame_info.frame_start_virt_addr()?.as_ptr_mut() as *mut *const u8, num_scratchpad_bufs)
        };
        let mut table: Pin<Box<[*const u8]>> = Pin::new(Box::from(table));

        // buffer
        let mut bufs = Vec::new();
        for sb in table.iter_mut() {
            // non-deallocate memory
            let sb_frame_info = bitmap::alloc_mem_frame(1)?;
            let buf_ptr = sb_frame_info.frame_start_virt_addr()?.as_ptr();
            let buf = unsafe {
                slice::from_raw_parts(buf_ptr as *const u8, PAGE_SIZE)
            };
            let buf: Pin<Box<[u8]>> = Pin::new(Box::from(buf));
            *sb = buf.as_ref().as_ptr();
            bufs.push(buf);
        }
        let scratchpad_bufs = ScratchpadBuffers {
            table,
            bufs,
        };
        trace!("{}: Scratchpad buffers initialized", driver_name);
        Ok(scratchpad_bufs)
    }

    fn init_dev_ctx(&mut self, scratchpad_bufs: ScratchpadBuffers) -> Result<()> {
        let driver_name = self.device_driver_info.name;

        // initialize device context
        let dcbaa = DeviceContextBaseAddressArray::new(scratchpad_bufs);
        self.ope_reg()?.as_mut().dcbaa_ptr.write(dcbaa.inner_mut_ptr());
        self.dcbaa = Some(dcbaa);
        trace!("{}: Device context base address array initialized", driver_name);

        Ok(())
    }

    fn init_primary_event_ring(&mut self) -> Result<()> {
        let driver_name = self.device_driver_info.name;

        self.primary_event_ring = Some(EventRing::new()?);
        let event_ring = self.primary_event_ring.as_mut().unwrap();
        let rt_reg = unsafe { self.rt_reg.as_mut().unwrap().get_unchecked_mut() };
        rt_reg.init_int_reg_set(0, event_ring)?;
        trace!("{}: Primary event ring initialized", driver_name);

        Ok(())
    }

    fn init_cmd_ring(&mut self) -> Result<()> {
        let driver_name = self.device_driver_info.name;

        self.cmd_ring = Some(CommandRing::default());
        let cmd_ring = self.cmd_ring.as_mut().unwrap();
        let ope_reg = unsafe { self.ope_reg.as_mut().unwrap().get_unchecked_mut() };
        ope_reg.set_cmd_ring_ctrl(cmd_ring);
        trace!("{}: Command ring initialized", driver_name);

        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        let driver_name = self.device_driver_info.name;
        self.ope_reg()?.as_mut().usb_cmd.set_run_stop(true);

        loop {
            trace!("{}: Waiting xHC...", driver_name);
            if !self.ope_reg()?.as_ref().usb_status.hchalted() {
                break;
            }
        }
        trace!("{}: xHC started", driver_name);
        trace!("{}: op_regs.usb_status: 0x{:x}", driver_name, self.ope_reg()?.as_ref().usb_status.read());
        trace!("{}: rt_regs.mfindex: 0x{:x}", driver_name, self.rt_reg()?.as_ref().mfindex());

        trace!("{}: portsc values for port {:?}", driver_name, self.portsc()?.port_range());
        let mut connected_port = None;
        for port in self.portsc()?.port_range() {
            if let Some(e) = self.portsc()?.get(port) {
                if e.ccs() {
                    connected_port = Some(port);
                }
            }
        }

        if let Some(port) = connected_port {
            info!("{}: Port {} is connected", driver_name, port);
            if let Some(portsc) = self.portsc()?.get(port) {
                portsc.reset_port();
                assert!(portsc.is_enabled());
                trace!("{}: Port {} has been reset and is enabled", driver_name, port);
            }
        }

        Ok(())
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
            trace!("{}: Processed TRB: 0x{:x}", driver_name, trb.trb_type());
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
