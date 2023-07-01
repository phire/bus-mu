use crate::pipeline::MemoryReq;


#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct CacheTag(u32);
impl CacheTag {
    #[inline]
    pub fn empty() -> CacheTag {
        CacheTag(0)
    }
    pub fn invalid() -> CacheTag {
        CacheTag(0xcccc_ccce)
    }

    pub fn new(tag: u32) -> CacheTag {
        CacheTag(tag & 0xffff_e000 | 1)
    }

    pub fn new_uncached(addr: u32) -> CacheTag {
        CacheTag(addr | 3)
    }

    #[inline]
    pub fn tag(&self) -> u32 {
         self.0 & 0xffff_e000
    }

    pub fn is_valid(&self) -> bool {
        (self.0 & 1) == 1
    }

    pub fn is_uncached(&self) -> bool {
        (self.0 & 3) == 3
    }

    pub fn get_uncached(&self) -> u32 {
        self.0 & 0xffff_fffc
    }

    pub fn is_dirty(&self) -> bool {
        (self.0 & 7) == 5
    }
}


#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum ICacheState {
    Normal,
    Filling,
    Refilled,
}

pub struct ICache {
    tag: [CacheTag; 512],
    data: [[u32; 8]; 512],
    pub state: ICacheState,
    /// The last uncached read
    uncached_read: (u32, CacheTag),
}

impl ICache {
    pub fn new() -> ICache {
        ICache {
            tag: [CacheTag::invalid(); 512],
            data: [[0; 8]; 512],
            state: ICacheState::Normal,
            uncached_read: (0, CacheTag::invalid()),
        }
    }
    pub fn fetch(&mut self, va: u64) -> (u32, CacheTag) {
        if va & 0xe000_0000 == 0xa000_0000 { // uncached via kseg1
            // We just return the last uncached read
            return self.uncached_read;
        }
        let word = va & 0x3;
        let line = (va >> 2) & 0x1ff;

        return (
            self.data[line as usize][word as usize],
            self.tag[line as usize],
        );
    }

    pub fn finish_uncached_read(&mut self, data: u32, addr: u32) {
        self.uncached_read = (data, CacheTag::new_uncached(addr));
        self.state = ICacheState::Refilled;
    }

    pub fn finish_fill(&mut self, data: [u32; 8], addr: u32) {
        let line = (addr >> 2) & 0x1ff;
        self.data[line as usize] = data;
        self.tag[line as usize] = CacheTag::new(addr);
        self.state = ICacheState::Refilled;
    }
}

pub struct DCache {
    data: [[u8; 16]; 512],
    tag: [CacheTag; 512],
    /// The last uncached read
    uncached_read: (u32, CacheTag),
}

impl DCache {
    pub fn new() -> DCache {
        DCache {
            data: [[0; 16]; 512],
            tag: [CacheTag::invalid(); 512],
            uncached_read: (0, CacheTag::invalid()),
        }
    }
    pub fn open(&mut self, addr: u64) -> DataCacheAttempt {
        let line = ((addr & 0x1ff0) >> 4) as usize;

        return DataCacheAttempt {
            tag: self.tag[line],
            line: line as u16,
            offset: (addr & 0xf) as u8
        };
    }
}

#[derive(Clone, Copy)]
pub struct DataCacheAttempt {
    tag: CacheTag,
    line: u16,
    offset: u8
}

impl DataCacheAttempt {
    pub fn empty() -> DataCacheAttempt {
        DataCacheAttempt {
            tag: CacheTag::invalid(),
            line: 0,
            offset: 0,
        }
    }

    pub fn is_hit(self, tlb_tag: CacheTag) -> bool {
        self.tag == tlb_tag && self.tag.is_valid()
    }

    pub fn do_miss(self, dcache: &DCache, tlb_tag: CacheTag) -> MemoryReq {
        let line = self.line as u32;
        let physical_address = tlb_tag.tag() | line;
        if self.tag.is_dirty() {
            let flush_physical_address = self.tag.tag() | line;
            MemoryReq::DCacheReplace(
                physical_address,
                flush_physical_address,
                dcache.data[self.line as usize],
            )
        } else {
            MemoryReq::DCacheFill(physical_address)
        }
    }

    pub fn write(self, dcache: &mut DCache, size: usize, data: u64) {
        // PERF: check if this function produces good code
        let start = self.offset as usize;
        let data_bytes = data.to_le_bytes();
        for i in 0..(size as usize) {
            dcache.data[self.line as usize][start + i] = data_bytes[i];
        }
    }

    pub fn read(self, dcache: &DCache, size: usize) -> u64 {
        // PERF: check if this function produces good code
        let start = self.offset as usize;
        let mut data_bytes = [0; 8];
        for i in 0..size {
            data_bytes[i] = dcache.data[self.line as usize][start + size - i];
        }

        return u64::from_le_bytes(data_bytes);
    }

}