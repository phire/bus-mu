

pub struct RegFile {
    regs: [u64; 32],
    pub hilo: [u64; 2],
}

impl RegFile {
    pub fn new() -> RegFile {
        RegFile {
            regs: [0; 32],
            hilo: [0; 2],
        }
    }
    pub fn read(&self, reg: u8) -> u64 {
        self.regs[reg as usize]
    }
    pub fn write(&mut self, reg: u8, val: u64) {
        if reg != 0 {
            self.regs[reg as usize] = val;
        }
    }
}