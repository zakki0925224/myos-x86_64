#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Utsname {
    pub sysname: [u8; 64],
    pub nodename: [u8; 64],
    pub release: [u8; 64],
    pub version: [u8; 64],
    pub machine: [u8; 64],
    pub domainname: [u8; 64],
}

impl Default for Utsname {
    fn default() -> Self {
        Self {
            sysname: [0; 64],
            nodename: [0; 64],
            release: [0; 64],
            version: [0; 64],
            machine: [0; 64],
            domainname: [0; 64],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Stat {
    pub size: usize, // file size (bytes)
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Sockaddr {
    pub sa_family: u16, // sa_family_t
    pub sa_data: [u8; 14],
}

impl Default for Sockaddr {
    fn default() -> Self {
        Self {
            sa_family: 0,
            sa_data: [0; 14],
        }
    }
}
