use super::cache::{CacheTag, ICacheAddress};


struct TlbEntry {
    vpn: u64,
    pfn: u32, // Pre-shifted
    _asid: u8,
    g: bool,
}

pub struct ITlb {
    entires: [TlbEntry; 2],
    lru: u8, // vr4300 user manual says:
             //    Micro-TLB "uses the least-recently- used (LRU) replacement algorithm"
}

impl ITlb {
    pub fn new() -> ITlb {
        ITlb {
            entires: [
                TlbEntry {
                    vpn: 0,
                    pfn: 0,
                    _asid: 0,
                    g: true,
                },
                TlbEntry {
                    vpn: 0,
                    pfn: 0,
                    _asid: 0,
                    g: true,
                },
            ],
            lru: 0,
        }
    }
    /// The CPU pipeline will call this every cycle for the instruction it's about to execute
    ///
    /// # Arguments
    ///
    /// * `va` - The 64bit (sign-extended) virtual address to translate
    /// * `state` - The current state of the memory subsystem
    ///
    /// # Returns
    ///
    /// Returns a cache tag if the
    ///
    pub fn translate(&mut self, va: u64) -> CacheTag {
        // PERF: put a single-entry cache in front of this?

        // These segments bypass TLB
        // HWTEST: Is TLB bypassing actually done here?
        //         It's theoretically possible that a JTLB lookup creates fake entries for these
        match va {
            0xffff_ffff_8000_0000..=0xffff_ffff_9fff_ffff => { // kseg0
                return CacheTag::new_uncached(va as u32 & 0x1fff_ffff);
            }
            0xffff_ffff_a000_0000..=0xffff_ffff_bfff_ffff => { // kseg1
                return CacheTag::new_uncached(va as u32 & 0x1fff_ffff);
            }
            _ => {}
        }

        // ACCURACY: Need to do permission checks
        //           But do we do it here, or when loading from JTLB?

        // micro-tlb is hardcoded to just two 4k pages
        let vpn = va >> 12;
        //let offset = (va & 0xfff) as u32;
        for (i, entry) in self.entires.iter().enumerate() {
            // TODO: Asid check
            // HWTEST: Does micro-tlb even check asid?
            //         Night make sense to only check it when loading from JTLB

            // TODO: handle pages marked as uncached
            let asid_match = true; // entry.asid == state.asid;
            if entry.vpn == vpn && (entry.g || asid_match) {
                self.lru = i as u8;
                return CacheTag::new(entry.pfn);
            }
        }
        return CacheTag::empty();
    }

    // pub fn miss(&mut self, _va: u64, _state: &MemSubsystemState) -> Option<u32> {
    //     // This is called when the pipeline didn't match on the previous translate
    //     // We need to load the correct TLB entry from JTLB (if it exists)

    //     // If it doesn't exist, the pipeline will raise a TLB miss exception, and the OS
    //     // is expected to update JTLB with the correct entry

    //     todo!("TLB miss")
    // }
}

pub struct TlbLookup {
    _addr: ICacheAddress,
}

impl TlbLookup {
    pub fn new(addr: ICacheAddress) -> TlbLookup {
        TlbLookup {
            _addr: addr,
        }
    }

    pub fn matches(self, tag: CacheTag) -> bool {
        todo!("matches {:?}", tag)
    }
}