use crate::{
    arch::IoPortAddress,
    device::{register_pollable, tty, CharDevice, Device, DeviceInfo, Pollable},
    error::{Error, Result},
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
};
use alloc::{sync::Arc, vec::Vec};

const NAME: &str = "ttyS0";

static SERIAL_PORT: Mutex<SerialPort> = Mutex::new(SerialPort::new());

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum ComPort {
    Com1 = 0x3f8,
    // Com2 = 0x2f8,
    // Com3 = 0x3e8,
    // Com4 = 0x2e8,
    // Com5 = 0x5f8,
    // Com6 = 0x4f8,
    // Com7 = 0x5e8,
    // Com8 = 0x4e8,
}

struct SerialPort {
    io_port_addr: Option<IoPortAddress>,
}

impl SerialPort {
    fn poll_normal(&mut self) -> Result<Option<u8>> {
        Ok(self.receive_data())
    }

    const fn new() -> Self {
        Self { io_port_addr: None }
    }

    fn receive_data(&self) -> Option<u8> {
        if !self.is_received_data() {
            return None;
        }

        let data = match self.io_port_addr() {
            Ok(port) => port.in8(),
            Err(_) => return None,
        };
        Some(data)
    }

    fn send_data(&self, data: u8) {
        // TODO: loop infinity on VirtualBox and actual device
        //while !self.is_transmit_empty() {}

        if let Ok(io_port_addr) = self.io_port_addr() {
            io_port_addr.out8(data);
        }
    }

    fn is_received_data(&self) -> bool {
        match self.io_port_addr() {
            Ok(port) => port.offset(5).in8() & 0x01 != 0,
            Err(_) => false,
        }
    }

    fn is_transmit_empty(&self) -> bool {
        match self.io_port_addr() {
            Ok(port) => port.offset(5).in8() & 0x20 != 0,
            Err(_) => false,
        }
    }

    fn io_port_addr(&self) -> Result<&IoPortAddress> {
        self.io_port_addr
            .as_ref()
            .ok_or(Error::NotInitialized.with_context("io_port_addr"))
    }
}

impl SerialPort {
    fn probe(&mut self) -> Result<()> {
        Ok(())
    }

    fn attach(&mut self) -> Result<()> {
        let io_port_addr = IoPortAddress::new(ComPort::Com1 as u32);

        io_port_addr.offset(1).out8(0x00); // IER - disable all interrupts
        io_port_addr.offset(3).out8(0x80); // LCR - enable DLAB
        io_port_addr.offset(0).out8(0x03); // DLL - set baud late 38400 bps
        io_port_addr.offset(1).out8(0x00); // DLM
        io_port_addr.offset(3).out8(0x03); // LCR - disable DLAB, 8bit, no parity, 1 stop bit
        io_port_addr.offset(2).out8(0xc7); // FCR - enable FIFO, clear TX/RX queues, 14byte threshold
        io_port_addr.offset(4).out8(0x0b); // MCR - IRQs enabled, RTS/DSR set
        io_port_addr.offset(4).out8(0x1e); // MCR - set loopback mode, test the serial chip
        io_port_addr.offset(0).out8(0xae); // RBR - test the serial chip (send 0xae)

        if io_port_addr.offset(0).in8() != 0xae {
            return Err(Error::InvalidData.with_context("serial port initialization"));
        }

        // if serial isn't faulty, set normal mode
        io_port_addr.offset(4).out8(0x0f);

        self.io_port_addr = Some(io_port_addr);
        Ok(())
    }
}

pub fn device_info() -> Result<DeviceInfo> {
    Ok(DeviceInfo::new(NAME))
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = SERIAL_PORT.try_lock()?;
    driver.probe()?;
    driver.attach()?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}

pub fn poll_normal() -> Result<()> {
    let received_data = match SERIAL_PORT.try_lock()?.poll_normal()? {
        Some(data) => data,
        None => return Ok(()),
    };

    tty::input(received_data as char)
}

pub fn send_data(data: u8) {
    let driver = unsafe { SERIAL_PORT.get_force_mut() };
    driver.send_data(data);
}

struct SerialDevice;

impl Device for SerialDevice {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo::new(NAME))
    }
}

impl CharDevice for SerialDevice {
    fn read(&self, _offset: usize, _max_len: usize) -> Result<Vec<u8>> {
        Err(Error::NotSupported.into())
    }

    fn write(&self, data: &[u8]) -> Result<()> {
        let driver = SERIAL_PORT.try_lock()?;

        for b in data {
            driver.send_data(*b);
        }

        Ok(())
    }
}

impl Pollable for SerialDevice {
    fn poll(&self) -> Result<()> {
        poll_normal()
    }
}

pub fn register() -> Result<()> {
    let device = Arc::new(SerialDevice);
    vfs::add_dev(device.clone())?;
    register_pollable(device)?;

    Ok(())
}
