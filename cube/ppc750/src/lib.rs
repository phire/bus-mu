
#![feature(portable_simd)]
#![feature(pattern_types)]
#![feature(pattern_type_macro)]
#![feature(pattern_type_range_trait)]
#![feature(const_trait_impl)]
#![allow(internal_features)]


pub mod instruction_queue;
pub mod completion;
pub mod branch_processing;
pub mod icache;


use instruction_queue::InstructionQueue;
use completion::*;
use branch_processing::*;
use icache::*;

#[derive(Clone, Copy)]
pub struct Pipeline {
    gprs: [u32; 32],
    fprs: [f64; 32],
    crs: [u8; 8],
    fetch_pc: u32,

    btic: BranchTargetInstructionCache,

    gp_renames: [u32; 6],
    fp_renames: [f64; 6],
    cr_renames: [u8; 6],

    iq: InstructionQueue,

    cq: CompletionQueue,
    bpu: BranchProcessingUnit,
    fpu: FloatingPointUnit,
    lsu: LoadStoreUnit,
    iu1: IntegerUnit,
    iu2: SimpleIntegerUnit,
    sru: SystemRegisterUnit,

    icache: ICache,
}

impl Pipeline {
    pub fn cycle(&mut self) {

        // BPU - final prediction
        //
        let branch_target = self.bpu.final_prediction();


        // Complete (writeback)
        let cq_len = self.cq.len();
        if cq_len != 0 {
            let cq0_finish_fn = self.cq.get().finish_fn;
            // Requirements to retire CQ[0] (from 750CL 6.6.1.3):
            //  - CQ[0] Must be finished
            //    -  which we handle by making finish_fn a nop until the instruction is finished
            //  - CQ[0] Must not be behind an unresolved branch
            //    - The manual says this, but I don't see any way this can be false, since the only
            //      way to resolve branch is to retire an existing completion entry resolving it.
            //  - CQ[0] Must not be exception
            //    - Which we handle by using an exceptional finish_fn

            // The whole checking logic is handled by updating finish_fn.
            // XXX-PERF: Should we inline the (cq_len == 0) check too?

            let continue_status = (cq0_finish_fn)(self, ContinueStatus::new());

            if continue_status.is_continue() && cq_len > 1 {
                // XXX-PERF: might make sense to tack this onto the end of the first finish_fn.

                // Requirement:
                // - CQ[1] must not follow an unresolved predicted branch
                //   - Handled by branch resolution finish_fn not allowing continues.
                //   - Really, if CQ0 finished, the the branch must be resolved.
                //     XXX-HWTEST: But I'm guessing it CQ[1] can't retire if it was resolved by CQ[0]?
                //   - According to the 604 manual, completion stops after taken branch to simplify PC updates.
                //     - And the 604 also stops completion before stores... which the 750 manual doesn't hint at all.
                // - CQ[1] must be an integer or load instruction
                //   - FPU/SFU finish_fn will check first and abort.
                // - GPR/FPR/CR updates must not exceed two each
                //   - Counts will be passed via continue_status, which handlers will check

                let cq1_finish_fn = self.cq.get().finish_fn;
                (cq1_finish_fn)(self, continue_status);
            }
        }

        // Instruction Fetch
        self.fetch_cycle(branch_target);

        // Instruction Cache
        self.icache.start_fetch(self.fetch_pc);

        // Branch Target Instruction Cache (BTIC)
        // See US5506976A for ARM's patent on BTIC
        // and US5740417A for Motorola's patent which describes a likely implementation for the 750.
    }

    fn fetch_cycle(&mut self, branch_target: Option<u32>) -> BusRequest {
        // fetches upto 4 instructions into the IQ, while simultaneously detecting branches.
        // Alternatively, fetch 2 instructions from BTIC.
        let free_slots = self.iq.free_slots();

        if free_slots == 0 {
            if let Some(target) = branch_target {
                // If no free slots, just ignore the BTIC lookup and update PC
                // It will be fetched from ICache when there is space in the queue.
                self.fetch_pc = target;
                self.btic.cancel();
            }
            return BusRequest::none();
        }

        let mut next_pc: u32;
        let instructions: &[u32];
        if let Some(target) = branch_target {
            next_pc = target;
            if let Some(btic_entry) = self.btic.lookup() {
                // BTIC hit - fetch from BTIC
                instructions = &btic_entry.instructions[0..free_slots.min(2)];
            } else {
                // BTIC miss - will simply fetch from ICache on next cycle.
                instructions = &[];
            }
        } else {
            next_pc = self.fetch_pc;
            instructions = match &self.icache.fetch() {
                FetchResult::Hit(insts) => insts,
                FetchResult::Miss => {
                    return BusRequest::icache(next_pc);
                }
            };
        }

        for &inst in instructions {
            self.bpu.decode(inst);
            self.iq.append(next_pc, &[inst]);
            next_pc += 4;
        }
        self.fetch_pc = next_pc;

        BusRequest::none()
    }
}

#[derive(Clone, Copy)]
pub struct IntegerUnit {
    rs: ReservationEntry,
}

#[derive(Clone, Copy)]
pub struct SimpleIntegerUnit {
    rs: ReservationEntry,
}

#[derive(Clone, Copy)]
pub struct ReservationEntry {
    completion_id : i8,
}

#[derive(Clone, Copy)]
pub struct FloatingPointUnit {

}

#[derive(Clone, Copy)]
pub struct LoadStoreUnit {

}

#[derive(Clone, Copy)]
pub struct SystemRegisterUnit {

}

pub struct BusRequest {
    address: u32,
    type_: BusRequestType,
}

impl BusRequest {
    pub fn none() -> Self {
        BusRequest { address: 0, type_: BusRequestType::None }
    }
    pub fn icache(address: u32) -> Self {
        BusRequest { address, type_: BusRequestType::ICacheFetch }
    }
}

pub enum BusRequestType {
    None,
    ICacheFetch,
    DataRead,
    DataWrite(u64),
}

#[derive(Clone, Copy, Debug)]
pub enum GPR {
    R0 = 0,
    R1 = 1,
    R2 = 2,
    R3 = 3,
    R4 = 4,
    R5 = 5,
    R6 = 6,
    R7 = 7,
    R8 = 8,
    R9 = 9,
    R10 = 10,
    R11 = 11,
    R12 = 12,
    R13 = 13,
    R14 = 14,
    R15 = 15,
    R16 = 16,
    R17 = 17,
    R18 = 18,
    R19 = 19,
    R20 = 20,
    R21 = 21,
    R22 = 22,
    R23 = 23,
    R24 = 24,
    R25 = 25,
    R26 = 26,
    R27 = 27,
    R28 = 28,
    R29 = 29,
    R30 = 30,
    R31 = 31,
}

// 32 floating point regs
#[derive(Clone, Copy, Debug)]
pub enum FPR {
    F0 = 0,
    F1 = 1,
    F2 = 2,
    F3 = 3,
    F4 = 4,
    F5 = 5,
    F6 = 6,
    F7 = 7,
    F8 = 8,
    F9 = 9,
    F10 = 10,
    F11 = 11,
    F12 = 12,
    F13 = 13,
    F14 = 14,
    F15 = 15,
    F16 = 16,
    F17 = 17,
    F18 = 18,
    F19 = 19,
    F20 = 20,
    F21 = 21,
    F22 = 22,
    F23 = 23,
    F24 = 24,
    F25 = 25,
    F26 = 26,
    F27 = 27,
    F28 = 28,
    F29 = 29,
    F30 = 30,
    F31 = 31,
}

// Condition Reg
#[derive(Clone, Copy, Debug)]
pub enum CR {
    CR0 = 0,
    CR1 = 1,
    CR2 = 2,
    CR3 = 3,
    CR4 = 4,
    CR5 = 5,
    CR6 = 6,
    CR7 = 7,
}

#[derive(Clone, Copy, Debug)]
pub enum WritebackDest {
    None,
    GPR(GPR),
    FPR(FPR),
    CR(CR),
}