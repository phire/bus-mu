


#[derive(Clone, Copy)]
pub struct InstructionQueue {
    queue: [QueueEntry; 6],
    count: u8,
}

impl InstructionQueue {
    pub fn append(&mut self, pc: u32, instructions: &[u32]) -> u32 {

        0

    }

    pub fn full(&self) -> bool {
        self.count == 6
    }

    pub fn free_slots(&self) -> usize {
        (6 - self.count as usize)
    }
}

#[derive(Clone, Copy)]
pub struct QueueEntry {
    instruction: u32,
    pc: u32,
    valid: bool,
}