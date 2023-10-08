use common::util::ByteMask8;


pub struct DBus {
    banks: [RambusBank; 4],
    /// Put all memory in a single, continuous allocation
    mem_data: Box<[u64; 4 * 1024 * 1024 / 8]>, // 4MB
}

impl DBus {
    pub fn new() -> Self {
        Self {
            banks: [RambusBank::new(), RambusBank::new(), RambusBank::new(), RambusBank::new()],
            mem_data: Box::new([0; 4 * 1024 * 1024 / 8]),
        }
    }

    fn access_column(&mut self, addr: u32) -> (u64, &mut u64) {
        let bank_id = (addr as usize >> 20) & 0x3;
        let row = (addr as usize >> 11) & 0x1ff;
        let _col = (addr as usize & 0x7ff) >> 3;
        let offset = (addr as usize & 0x3fffff) >> 3;

        let bank = &mut self.banks[bank_id];

        let mut cycles = 1;

        if bank.sensed_row != Some(row as u16) {
            cycles += bank.open_row(row as u16);
        }

        let data = &mut self.mem_data[offset];

        (cycles, data)
    }


    pub fn write_qword_masked(&mut self, addr: u32, data: u64, mask: ByteMask8) -> u64 {
        assert!(addr & 0x7 == 0, "unaligned qword write");

        let (cycles, mem) = self.access_column(addr);
        mask.masked_insert(mem, data);

        cycles
    }

    pub fn write_qword(&mut self, addr: u32, data: u64) -> u64 {
        assert!(addr & 0x7 == 0, "unaligned qword write");

        let (cycles, mem) = self.access_column(addr);
        *mem = data;

        cycles
    }

    pub fn read_qword(&mut self, addr: u32) -> (u64, u64) {
        assert!(addr & 0x7 == 0, "unaligned qword write");

        let (cycles, mem) = self.access_column(addr);
        (cycles, *mem)
    }
}


pub struct RambusBank {
    sensed_row: Option<u16>,
    dirty: bool,
}

impl RambusBank {
    pub fn new() -> Self {
        Self {
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
