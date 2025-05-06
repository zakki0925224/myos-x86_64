use super::context::input::InputContext;
use crate::{
    arch::{addr::VirtualAddress, mmio::Mmio, volatile::Volatile},
    error::{Error, Result},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigState {
    NotConnected,
    Reset,
    Enabled,
    AddressingDevice,
    InitializingDevice,
    ConfiguringEndpoints,
    Configured,
}

#[derive(Debug)]
pub struct Port {
    port_id: usize,
    pub slot_id: Option<usize>,
    pub config_state: ConfigState,
    pub input_context_reg: Option<Mmio<Volatile<InputContext>>>,
    output_context_reg: Option<Mmio<Volatile<u8>>>,
}

impl Port {
    pub fn new(port_id: usize) -> Self {
        Self {
            port_id,
            slot_id: None,
            config_state: ConfigState::NotConnected,
            input_context_reg: None,
            output_context_reg: None,
        }
    }

    pub fn port_id(&self) -> usize {
        self.port_id
    }

    pub fn set_input_context_reg(&mut self, reg_ptr: *mut Volatile<InputContext>) {
        self.input_context_reg = Some(unsafe { Mmio::from_raw(reg_ptr) });
    }

    pub fn set_output_context_reg(&mut self, reg_ptr: *mut Volatile<u8>) {
        self.output_context_reg = Some(unsafe { Mmio::from_raw(reg_ptr) });
    }

    pub fn read_input_context(&self) -> Result<InputContext> {
        if let Some(ref reg) = self.input_context_reg {
            Ok(reg.as_ref().read())
        } else {
            Err(Error::Failed("Input context register not set"))
        }
    }

    pub fn write_input_context(&mut self, context: InputContext) -> Result<()> {
        if let Some(ref mut reg) = self.input_context_reg {
            unsafe { reg.get_unchecked_mut().write(context) };
            Ok(())
        } else {
            Err(Error::Failed("Input context register not set"))
        }
    }

    pub fn input_context_base_addr(&self) -> Result<VirtualAddress> {
        if self.input_context_reg.is_none() {
            return Err(Error::Failed("Input context register not set"));
        }

        Ok((self.input_context_base_addr().as_ref().unwrap() as *const _ as u64).into())
    }
}
