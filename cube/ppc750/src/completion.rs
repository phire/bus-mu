use crate::WritebackDest;



#[derive(Clone, Copy)]
pub struct CompletionQueue {
    queue: [CompletionEntry; 6],
    pub head: Count6,
    len: usize,
    pub resolved_tail: Count6,
}

impl CompletionQueue {
    pub fn len(&self) -> usize {
        return self.len;
    }

    pub fn len_resolved(&self) -> usize {
        let head = self.head as usize;
        let resolved_tail = self.resolved_tail as usize;

        head.checked_sub(resolved_tail).unwrap_or_else(|| 6 - (head - resolved_tail))
    }

    pub fn get(&self) -> &CompletionEntry {
        &self.queue[self.head as usize]
    }


}

#[derive(Clone, Copy)]
pub struct CompletionEntry {
    pc: u32,
    finished: bool,
    wb_a_dest: WritebackDest,
    wb_a_rename: Count6,
    pub finish_fn: fn(&mut crate::Pipeline, status: ContinueStatus) -> ContinueStatus,
}

pub struct ContinueStatus(u32);
impl ContinueStatus {
    pub fn new() -> Self {
        ContinueStatus(0)
    }

    pub fn is_continue(&self) -> bool {
        self.0 != 0
    }

    fn blocked() -> Self {
        ContinueStatus(0)
    }

    fn with(gprs: u32, fprs: u32, crs: u32) -> Self {
        ContinueStatus(1 | (gprs << 1) | (fprs << 3) | (crs << 5))
    }
}


#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Count6 {
    E0 = 0,
    E1 = 1,
    E2 = 2,
    E3 = 3,
    E4 = 4,
    E5 = 5,
}

impl Count6 {
    pub fn next(&self) -> Self {
        match self {
            Count6::E0 => Count6::E1,
            Count6::E1 => Count6::E2,
            Count6::E2 => Count6::E3,
            Count6::E3 => Count6::E4,
            Count6::E4 => Count6::E5,
            Count6::E5 => Count6::E0,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Count6::E0 => Count6::E5,
            Count6::E1 => Count6::E0,
            Count6::E2 => Count6::E1,
            Count6::E3 => Count6::E2,
            Count6::E4 => Count6::E3,
            Count6::E5 => Count6::E4,
        }
    }
}
