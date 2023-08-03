use super::instructions::MIPS_REG_NAMES;

pub struct RegFile {
    regs: [u64; 32],
    bypass_reg: u8,
    bypass_val: Option<u64>,
    hazard: bool,
}

impl RegFile {
    pub(super) fn new() -> RegFile {
        RegFile {
            regs: [0; 32],
            bypass_reg: 0,
            bypass_val: None,
            hazard: false,
        }
    }
    pub(super) fn read(&mut self, reg: u8) -> u64 {
        if reg == self.bypass_reg {
            match self.bypass_val {
                Some(val) => {
                    println!("Bypassing {} = {:#08x}", MIPS_REG_NAMES[reg as usize], val);
                    return val;
                },
                None => {
                    println!("Hazzard detected {}", MIPS_REG_NAMES[reg as usize]);
                    self.hazard = true;
                    return 0;
                },
            }
        }
        println!("Reading {} = {:#08x}", MIPS_REG_NAMES[reg as usize], self.regs[reg as usize]);
        self.regs[reg as usize]
    }
    pub(super) fn write(&mut self, reg: u8, val: u64) {
        println!("Writing {} = {:#08x}", MIPS_REG_NAMES[reg as usize], val);
        if reg != 0 {
            self.regs[reg as usize] = val;
        }
    }
    pub(super) fn bypass(&mut self, reg: u8, val: Option<u64>) {
        self.hazard = false;
        if reg == 0 {
            self.bypass_reg = 0xff;
        } else {
            self.bypass_reg = reg;
            self.bypass_val = val;
        }
    }
    pub fn hazard_detected(&self) -> bool {
        self.hazard
    }
}
