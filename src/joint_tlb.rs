// struct JTlb {
//     entires: [TlbEntry; 32],
//     random: u8,
// }

// impl JTlb {
//     pub fn translate(&mut self, va: u64, asid: u8) -> Option<u32> {
//         // PERF: put a hash-map in front of this?

//         let vpn = va >> 12;
//         let offset = (va & 0xfff) as u32;
//         for (i, entry) in self.entires.iter().enumerate() {
//             //
//             if entry.vpn == vpn && {
//                 self.lru = i;
//                 return Some(entry.pfn | offset);
//             }
//         }
//         return None;
//     }
// }