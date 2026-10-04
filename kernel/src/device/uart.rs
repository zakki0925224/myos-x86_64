use crate::{
    arch::IoPortAddress,
    device::{tty, DeviceInfo, Driver},
    error::{Error, Result},
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
};

const NAME: &str = "ttyS0";

static UART_DRIVER: Mutex<UartDriver> = Mutex::new(UartDriver::new());

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

struct UartDriver {
    io_port_addr: Option<IoPortAddress>,
}

impl UartDriver {
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

impl Driver for UartDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
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

    fn poll(&mut self) -> Result<()> {
        let received_data = match self.receive_data() {
            Some(data) => data,
            None => return Ok(()),
        };

        tty::input(received_data as char)
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        for b in data {
            self.send_data(*b);
        }

        Ok(())
    }
}

pub fn probe_and_attach() -> Result<()> {
    UART_DRIVER.try_lock()?.attach()?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}

pub fn register_dev() -> Result<()> {
    vfs::add_dev(&UART_DRIVER)?;
    Ok(())
}

pub fn poll_normal() -> Result<()> {
    UART_DRIVER.try_lock()?.poll()
}

pub fn send_data(data: u8) {
    let driver = unsafe { UART_DRIVER.get_force_mut() };
    driver.send_data(data);
}
