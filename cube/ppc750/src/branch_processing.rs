

#[derive(Clone, Copy)]
pub struct BranchProcessingUnit {
}

impl BranchProcessingUnit {
    pub fn final_prediction(&self) -> Option<u32> {
        // TODO
        None
    }

    pub fn decode(&mut self, instruction: u32) -> BranchPrediction {
        // From what I can tell, the BPU can decode enough of the instruction to not only know it
        // is a branch, but determine the branch target address.
        // Which means it has an 14 bit adder for relative branches.
        // The adder probably always runs (on all 4 instructions) even when it's not a branch.

        BranchPrediction::Fallthrough
    }
}

pub enum BranchPrediction {
    Fallthrough,
    Taken(u32),
    Ta


}

#[derive(Clone, Copy)]
pub struct BranchTargetInstructionCache {
    btic_address: Option<u32>,
    array: [[BticEntry; 4]; 16],
}

#[derive(Clone, Copy)]
pub struct BticEntry {
    pub tag: u32,
    pub instructions: [u32; 2],
}

impl BranchTargetInstructionCache {
    pub fn lookup(&mut self) -> Option<&BticEntry> {
        if let Some(branch_address) = self.btic_address {
            self.btic_address = None; // Clear the BTIC address after lookup
            let set = (branch_address as usize >> 2) & 0xF; // 16 sets
            for way in self.array[set].iter() {
                // ACCURACY: real hardware only checks the upper 26 bits
                if way.tag == branch_address {
                    return Some(way);
                }
            }
        }
        None
    }

    pub fn cancel(&mut self) {
        self.btic_address = None;
    }
}