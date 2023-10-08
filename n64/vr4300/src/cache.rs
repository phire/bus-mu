use common::util::ByteMask8;

use super::pipeline::MemoryReq;

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct CacheTag {
    val: u32,
}
impl CacheTag {
    #[inline]
    pub fn empty() -> CacheTag {
        CacheTag{ val: 0 }
    }
    pub fn invalid() -> CacheTag {
        CacheTag{ val: 0xcccc_ccce }
    }

    pub fn new(tag: u32) -> CacheTag {
        // Both DCache and ICache use bits 31:12 as the tag
        // This does cause some overlap with the line. ICache uses bits 13:5 and DCache uses bits 12:4
        CacheTag{ val: tag & 0xffff_f000 | 1 }
    }

    pub fn new_uncached(addr: u32) -> CacheTag {
        CacheTag{ val: addr | 3 }
    }

    #[inline]
    pub fn tag(&self) -> u32 {
         self.val & 0xffff_f000
    }

    pub fn is_valid(&self) -> bool {
        (self.val & 1) == 1
    }

    pub fn is_uncached(&self) -> bool {
        (self.val & 3) == 3
    }

    pub fn get_uncached(&self) -> u32 {
        self.val & 0xffff_fffc
    }

    pub fn is_dirty(&self) -> bool {
        (self.val & 7) == 5
    }
}

pub struct ICacheLine {
    val: u16
}

impl ICacheLine {
    pub fn new(addr: u32) -> ICacheLine {
        ICacheLine{ val: addr as u16 & 0x3fe0 }
    }

    pub fn line(self) -> usize {
        (self.val >> 5) as usize
    }
}

#[derive(Clone, Copy)]
pub struct ICacheAddress {
    val: u32
}

impl ICacheAddress {
    pub fn new(addr: u32) -> ICacheAddress {
        ICacheAddress{ val: addr & 0xffff_fffc }
    }

    pub fn tag(self) -> CacheTag {
        CacheTag::new(self.val)
    }

    pub fn line(self) -> usize {
        ICacheLine::new(self.val).line()
    }

    pub fn offset(self) -> usize {
        (self.val & 0x1c) as usize >> 2
    }

    pub fn value(self) -> u32 {
        self.val
    }
}

pub struct DCacheLine {
    val: u16
}

impl DCacheLine {
    pub fn new(addr: u32) -> DCacheLine {
        DCacheLine{ val: addr as u16 & 0x1ff0 }
    }

    pub fn line(self) -> usize {
        (self.val >> 4) as usize
    }
}

pub struct DCacheAddress {
    val: u32
}

impl DCacheAddress {
    pub fn new(addr: u32) -> DCacheAddress {
        DCacheAddress{ val: addr }
    }

    pub fn tag(self) -> CacheTag {
        CacheTag::new(self.val)
    }

    pub fn line(self) -> usize {
        DCacheLine::new(self.val).line()
    }

    pub fn offset(self) -> usize {
        (self.val & 0xf) as usize
    }

    pub fn value(self) -> u32 {
        self.val
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
}

impl ICache {
    pub fn new() -> ICache {
        ICache {
            tag: [CacheTag::invalid(); 512],
            data: [[0; 8]; 512],
        }
    }
    pub fn fetch(&mut self, va: u64) -> (u32, CacheTag) {
        let addr = ICacheAddress::new(va as u32);

        return (
            self.data[addr.line()][addr.offset()],
            self.tag[addr.line()],
        );
    }

    pub fn finish_fill(&mut self, line: usize, tag: CacheTag, data: [u64; 4]) {
        for (i, double) in data.into_iter().enumerate() {
            self.data[line][i << 1] = (double >> 32) as u32;
            self.data[line][(i << 1) + 1] = double as u32;
        }
        self.tag[line] = tag;
    }
}

pub struct DCache {
    data: [[u64; 2]; 512],
    tag: [CacheTag; 512],
}

impl DCache {
    pub fn new() -> DCache {
        DCache {
            data: [[0; 2]; 512],
            tag: [CacheTag::invalid(); 512],
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

    pub fn do_miss(self, dcache: &DCache, tlb_tag: CacheTag, size: u8, is_store: bool, value: u64, mask: ByteMask8) -> MemoryReq {
        let line = self.line as u32;
        let physical_address = tlb_tag.tag() | ((line << 4) & 0xfff);

        if tlb_tag.is_uncached() {
            let full_physical_address = physical_address | self.offset as u32;
            match (is_store, size) {
                (false, 8) => MemoryReq::UncachedDataReadDouble(full_physical_address),
                (false, _) => MemoryReq::UncachedDataReadWord(full_physical_address),
                (true, 8) => MemoryReq::UncachedDataWriteDouble(full_physical_address, value, mask),
                (true, _) => MemoryReq::UncachedDataWriteWord(full_physical_address, value, mask),
            }
        } else if self.tag.is_dirty() {
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

    pub fn write(self, dcache: &mut DCache, value: u64, mask: ByteMask8) {
        let dword_offset = self.offset as usize >> 3;
        let line = self.line as usize;
        mask.masked_insert(&mut dcache.data[line][dword_offset], value);
    }

    pub fn read(self, dcache: &DCache) -> u64 {
        let dword_offset = self.offset as usize >> 3;
        let line = self.line as usize;
        dcache.data[line][dword_offset]
    }

    pub fn finish_fill(&mut self, dcache: &mut DCache, new_tag: CacheTag, data: [u64; 2]) {
        self.tag = new_tag;
        dcache.data[self.line as usize] = data;
        dcache.tag[self.line as usize] = new_tag;
    }

    pub fn offset(&self) -> u8 {
        self.offset
    }

}