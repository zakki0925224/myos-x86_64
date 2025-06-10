use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum IomsgCommand {
    RemoveComponent = 0x80000000,
    CreateComponentWindow = 0x80000001,
    CreateComponentImage = 0x80000002,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct IomsgHeader {
    pub cmd_id: u32,
    pub payload_size: u32,
}

impl IomsgHeader {
    pub fn new(cmd: IomsgCommand, payload_size: u32) -> Self {
        Self {
            cmd_id: cmd as u32,
            payload_size,
        }
    }

    pub fn is_valid(&self) -> bool {
        (self.cmd_id & 0x80000000) != 0 && self.payload_size > 0
    }

    pub fn cmd(&self) -> Result<IomsgCommand> {
        match self.cmd_id {
            0x80000000 => Ok(IomsgCommand::RemoveComponent),
            0x80000001 => Ok(IomsgCommand::CreateComponentWindow),
            0x80000002 => Ok(IomsgCommand::CreateComponentImage),
            _ => Err(Error::Failed("Invalid command ID")),
        }
    }
}
