
pub struct DBus {
    banks: [RambusBank; 4],
}

impl DBus {
    pub fn new() -> Self {
        Self {
            banks: [RambusBank::new(), RambusBank::new(), RambusBank::new(), RambusBank::new()],
        }
    }

    fn get_slice<const COUNT: usize>(&mut self, addr: u32) -> (u64, &mut [u8]) {
        let bank_id = (addr as usize >> 20) & 0x3;
        let row = (addr as usize >> 11) & 0x1ff;
        let col = addr as usize & 0x7ff;

        let bank = &mut self.banks[bank_id];

        let mut cycles = COUNT as u64 / 8;

        if bank.sensed_row != Some(row as u16) {
            cycles += bank.open_row(row as u16);
        }
        // TODO: What happens if we cross a row boundary?
        if (col + COUNT) > 0x800 {
            todo!("Crossing row boundary");
        }

        let bank_addr = row << 11 | col;
        let data = &mut bank.mem[bank_addr..bank_addr + COUNT];

        (cycles, data)
    }

    pub fn read_qwords<const COUNT: usize>(&mut self, addr: u32) -> (u64, [u8; COUNT]) {
        let (cycles, mem) = self.get_slice::<COUNT>(addr);

        let data = mem[..COUNT].try_into().unwrap();

        (cycles, data)
    }

    pub fn write_qwords<const COUNT: usize>(&mut self, addr: u32, data: [u8; COUNT]) -> u64 {
        let (cycles, mem) = self.get_slice::<COUNT>(addr);

        mem.copy_from_slice(&data);

        cycles
    }
}


pub struct RambusBank {
    mem: Box<[u8; 1024 * 1024]>, // 1MB
    sensed_row: Option<u16>,
    dirty: bool,
}

impl RambusBank {
    pub fn new() -> Self {
        Self {
            mem:Box::new([0; 1024 * 1024]),
            sensed_row: None,
            dirty: false,
        }
    }

    fn open_row(&mut self, row: u16) -> u64 {
        // TODO: These times are probably all wrong. I just wanted some times.

        // tRowOverHead
        let mut time = 2;

        if self.sensed_row != Some(row) {
            if self.dirty {
                // tRowExprestore
                time += 2;
                self.dirty = false;
            }
             //  tRowPrecharge
            time += 2;
        }
        self.sensed_row = Some(row);

        // tRowSense
        time += 2;

        time
    }
}
