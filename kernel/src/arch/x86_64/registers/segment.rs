use super::Register;
use core::arch::asm;

pub struct Cs(u16);

impl Register<u16> for Cs {
    fn read() -> Self {
        let cs;

        unsafe {
            asm!("mov {:x}, cs", out(reg) cs);
        }

        Self(cs)
    }

    fn write(&self) {
        unsafe {
            asm!(
                "lea rax, [rip + 55f]",
                "push cx",
                "push rax",
                "ljmp [rsp]",
                "55:",
                "add rsp, 8 + 2",
                in("cx") self.0
            );
        }
    }

    fn raw(&self) -> u16 {
        self.0
    }

    fn set_raw(&mut self, value: u16) {
        self.0 = value;
    }
}

pub struct Ss(u16);

impl Register<u16> for Ss {
    fn read() -> Self {
        let ss;

        unsafe {
            asm!("mov {:x}, ss", out(reg) ss);
        }

        Self(ss)
    }

    fn write(&self) {
        unsafe {
            asm!("mov ss, ax", in("ax") self.0);
        }
    }

    fn raw(&self) -> u16 {
        self.0
    }

    fn set_raw(&mut self, value: u16) {
        self.0 = value;
    }
}

pub struct Ds(u16);

impl Register<u16> for Ds {
    fn read() -> Self {
        let ds;

        unsafe {
            asm!("mov {:x}, ds", out(reg) ds);
        }

        Self(ds)
    }

    fn write(&self) {
        unsafe {
            asm!("mov ds, ax", in("ax") self.0);
        }
    }

    fn raw(&self) -> u16 {
        self.0
    }

    fn set_raw(&mut self, value: u16) {
        self.0 = value;
    }
}

pub struct Es(u16);

impl Register<u16> for Es {
    fn read() -> Self {
        let es;

        unsafe {
            asm!("mov {:x}, es", out(reg) es);
        }

        Self(es)
    }

    fn write(&self) {
        unsafe {
            asm!("mov es, ax", in("ax") self.0);
        }
    }

    fn raw(&self) -> u16 {
        self.0
    }

    fn set_raw(&mut self, value: u16) {
        self.0 = value;
    }
}

pub struct Fs(u16);

impl Register<u16> for Fs {
    fn read() -> Self {
        let fs;

        unsafe {
            asm!("mov {:x}, fs", out(reg) fs);
        }

        Self(fs)
    }

    fn write(&self) {
        unsafe {
            asm!("mov fs, ax", in("ax") self.0);
        }
    }

    fn raw(&self) -> u16 {
        self.0
    }

    fn set_raw(&mut self, value: u16) {
        self.0 = value;
    }
}

pub struct Gs(u16);

impl Register<u16> for Gs {
    fn read() -> Self {
        let gs;

        unsafe {
            asm!("mov {:x}, gs", out(reg) gs);
        }

        Self(gs)
    }

    fn write(&self) {
        unsafe {
            asm!("mov gs, ax", in("ax") self.0);
        }
    }

    fn raw(&self) -> u16 {
        self.0
    }

    fn set_raw(&mut self, value: u16) {
        self.0 = value;
    }
}

pub fn set_ds_es_fs_gs(value: u16) {
    let mut ds = Ds::read();
    ds.set_raw(value);
    ds.write();

    let mut es = Es::read();
    es.set_raw(value);
    es.write();

    let mut fs = Fs::read();
    fs.set_raw(value);
    fs.write();

    let mut gs = Gs::read();
    gs.set_raw(value);
    gs.write();
}

pub fn set_ss_cs(ss_value: u16, cs_value: u16) {
    let mut ss = Ss::read();
    ss.set_raw(ss_value);
    ss.write();

    let mut cs = Cs::read();
    cs.set_raw(cs_value);
    cs.write();
}
