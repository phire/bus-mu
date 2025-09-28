#![allow(incomplete_features)]
#![feature(explicit_tail_calls)]

pub use paste::paste;


pub struct Bytecode<Op> {
    instructions: Vec<Op>,
    consts: Vec<u8>,
}


pub trait IntoBytecode<Op> {
    fn into_bytecode(self) -> InstructionPrototype<Op>;
}

#[derive(Copy, Clone)]
struct RegRef {
    index: u8,
}

impl RegRef {
    pub fn new(index: u8) -> RegRef {
        RegRef { index }
    }
}

pub enum ArgValue {
    Reg(u8),
    Imm(u64),
    Fixed(u8),
}

pub enum Dest {
    None,
    Reg(u8),
    Fixed(u8),
}

pub struct InstructionPrototype<Op> {
    op: Op,
    effects: bool,
    result: Dest,
    args: [Option<ArgValue>; 3], // We (currently) support upto 3 arguments per bytecode op
}

impl<Op> IntoBytecode<Op> for InstructionPrototype<Op> {
    fn into_bytecode(self) -> InstructionPrototype<Op> {
        self
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SSARef(Option<std::num::NonZeroU16>);

impl SSARef {
    fn none() -> Self {
        SSARef(None)
    }
    fn new(index: u16) -> Self {
        SSARef(std::num::NonZeroU16::new(index))
    }

    fn is_some(&self) -> bool {
        self.0.is_some()
    }

    fn is_none(&self) -> bool {
        self.0.is_none()
    }

    fn get(&self) -> u16 {
        self.0.map(|v| v.get() as u16).unwrap_or(0)
    }

    fn or_insert_with<F>(&mut self, f: F) -> &Self
    where
        F: FnOnce() -> SSARef,
    {
        match self.0 {
            Some(_) => self,
            None => {
                self.0 = f().0;
                self
            }
        }
    }
}

impl From<SSARef> for usize {
    fn from(value: SSARef) -> Self {
        value.0.map(|v| v.get() as usize).unwrap_or(0)
    }
}

#[derive(Copy, Clone)]
enum SSASource {
    None,
    RegRead(u8),
    RegWrite(u8),
    Fixed(u8),
    Imm(u64),
}


#[derive(Copy, Clone)]
enum SSAAloc {
    None,
    State(u8), // Direct to state struct
    Reg0,
    Reg1,
    Reg2,
}

struct SSANode {
    value: SSASource,
    created: u16,
    killed: u16,
    read_count: u16,
    last_read: u16,
    alloc: SSAAloc,
}

impl Default for SSANode {
    fn default() -> Self {
        SSANode {
            value: SSASource::None,
            created: 0,
            killed: 0,
            read_count: 0,
            last_read: 0,
            alloc: SSAAloc::None
        }
    }
}

impl SSANode {
    fn new(value: SSASource, idx: u16) -> SSANode {
        let mut node = SSANode {
            value,
            created: idx,
            ..Default::default()
        };
        match node.value {
            SSASource::Fixed(0) => node.alloc = SSAAloc::Reg0,
            SSASource::Fixed(1) => node.alloc = SSAAloc::Reg1,
            SSASource::Fixed(2) => node.alloc = SSAAloc::Reg2,
            _ => {}
        }

        node
    }

    fn mark_read(&mut self, idx: u16) -> &mut Self {
        self.read_count += 1;
        self.last_read = idx;
        self
    }

    fn mark_dead(&mut self, idx: u16) -> &mut Self {
        self.killed = idx;
        self
    }
}

pub struct SSAInstruction<Op> {
    op: Op,
    effects: bool,
    result: SSARef,
    args: [SSARef; 3],
}

pub struct BytecodeBuilder<Op> {
    instructions: Vec<SSAInstruction<Op>>,
    ssa: Vec<SSANode>,
    reg_map: [SSARef; 256],
    fixed: [SSARef; 3],
}

impl<Op> BytecodeBuilder<Op> {
    pub fn new() -> BytecodeBuilder<Op> {
        BytecodeBuilder {
            instructions: Vec::new(),
            ssa: vec![SSANode::default()], // Dummy node, so we can use 0 as null
            reg_map: [SSARef::none(); 256],
            fixed: [SSARef::none(); 3],
        }
    }

    pub fn push<O: IntoBytecode<Op>>(&mut self, inst: O) -> Result<(), &'static str> {
        let idx : u16 = self.instructions.len().try_into().map_err(|_| "Too many instructions")?;
        let proto = inst.into_bytecode();

        let args = {
            let mut args = [SSARef::none(); 3];
            for (i, arg) in proto.args.into_iter().enumerate() {
                args[i] = match arg {
                    None => { SSARef::none() }
                    Some(ArgValue::Reg(r)) => {
                        let ssa = self.reg_map[r as usize]
                            .or_insert_with(|| {
                                self.ssa.push(SSANode::new(SSASource::RegRead(r), idx));
                                SSARef::new(self.ssa.len() as u16 - 1)
                            }).clone();

                        self.get_ssa_mut(ssa).mark_read(idx);
                        ssa
                    }
                    Some(ArgValue::Imm(val)) => {
                        // TODO: Should we be deduplicating immediates?
                        let ssa = self.new_ssa(SSASource::Imm(val));
                        self.get_ssa_mut(ssa)
                            .mark_read(idx) // read immediately
                            .mark_dead(idx); // Kill immediately (because we don't de-duplicate consts)

                        ssa
                    }
                    Some(ArgValue::Fixed(r)) if r < 3 => {
                        let ssa = self.fixed[r as usize];
                        if ssa.is_none() { return Err("Fixed register read before write"); }
                        self.get_ssa_mut(ssa).mark_read(idx);
                        ssa
                    }
                    Some(ArgValue::Fixed(_)) => return Err("Fixed register out of range"),
                }
            }
            args
        };

        let result = match proto.result {
            Dest::None => SSARef::none(),
            Dest::Reg(r) => {
                self.overwrite(|slf| &mut slf.reg_map[r as usize], SSASource::RegWrite(r))
            }
            Dest::Fixed(r) if r < 3 => {
                self.overwrite(|slf| &mut slf.fixed[r as usize], SSASource::Fixed(r))
            }
            Dest::Fixed(_) => return Err("Fixed register out of range"),
        };

        self.instructions.push(SSAInstruction { op: proto.op, effects: proto.effects, result, args });

        Ok(())
    }


    fn overwrite<F: Fn(&mut Self) -> &mut SSARef>(&mut self, mapping: F, value: SSASource) -> SSARef {
        let new_ssa = self.new_ssa(value);

        let old_ssa = std::mem::replace(mapping(self), new_ssa.clone());
        if old_ssa.is_some() {
            // Overwrite a previous ssa.
            let idx = self.instructions.len() as u16;
            self.get_ssa_mut(old_ssa).mark_dead(idx);
        }
        new_ssa
    }

    fn new_ssa(&mut self, value: SSASource) -> SSARef {
        self.ssa.push(SSANode::new(value, self.instructions.len() as u16));
        SSARef::new(self.ssa.len() as u16 - 1)
    }

    fn get_ssa(&self, r: SSARef) -> &SSANode {
        &self.ssa[usize::from(r)]
    }

    fn get_ssa_mut(&mut self, r: SSARef) -> &mut SSANode {
        &mut self.ssa[usize::from(r)]
    }

    pub fn finish(self) -> Result<Bytecode<Op>, &'static str> {
        //let mut active = [None; 3];
        //let bytecode = Vec::new();
        //let consts = Vec::new();

        for (i, inst) in self.instructions.iter().enumerate() {
            let mut args = [SSAAloc::None; 3];
            // reads
            for arg in inst.args.into_iter().filter(|x| x.is_some()) {
                let ssa = self.get_ssa(arg);
                match ssa.alloc {
                    SSAAloc::None => {
                        match ssa.value {
                            SSASource::RegRead(0) => args[i] = SSAAloc::Reg0,
                            SSASource::RegRead(1) => args[i] = SSAAloc::Reg1,
                            SSASource::RegRead(2) => args[i] = SSAAloc::Reg2,
                            _ => { todo!("?") }
                        }
                    }
                    alloc => args[i] = alloc,
                }
            }

            // let result = if inst.result.is_some() {

            // }
        }

        unimplemented!();

        // Bytecode {
        //     instructions: bytecode,
        //     consts,
        // }
    }
}

#[macro_export]
macro_rules! tailspin {
    (
        @state($state_ty:ty)

        $(@op( $op_name:ident $( $stt:tt )+ ))*
    ) => {
        type StateType = $state_ty;

        #[derive(Copy, Clone)]
        struct ExitCode(u32);

        #[derive(Copy, Clone)]
        struct Op(u32);

        #[derive(Copy, Clone)]
        pub struct Handlers(*const Handler);

        type Handler = tailspin!(@argfn fn(args: Args) -> ExitCode);

        trait BytecodeOp {
            const OPCODE: Ops;
            fn get_handlers() -> Vec<Handler>;
        }

        tailspin!( @munch(0usize, ) $(@op($op_name $( $stt )+ ))* @enum(Ops) );

        struct Interpreter {
            handlers: [Handler; 0x10000],
        }

        tailspin!(@argfn fn unimplemented_op(args: Args) -> ExitCode {
            panic!("Unimplemented operation {:x}", args.op.0);
        });

        impl Interpreter {
            fn new() -> Interpreter {
                let mut handlers = vec![];

                let mut insert = |handles| {
                    handlers.extend(handles);
                };

                $( insert(<$op_name as BytecodeOp>::get_handlers()); )*

                assert_eq!(handlers.len(), NUM_OPS, "Handlers count mismatch");
                assert!(handlers.len() <= 0x10000, "Too many handlers, max is 0x10000");


                handlers.resize(0x10000, unimplemented_op as Handler);

                Interpreter { handlers: handlers.try_into().unwrap() }
            }

            unsafe fn run(&self, pp: *const Op, cp: *const u8, state: &mut $state_ty) -> ExitCode {
                let mut args = Args {
                    pp,
                    handlers: Handlers(self.handlers.as_ptr()),
                    op: Op(0), // Dummy op
                    cp,
                    r0: state.regs[0],
                    r1: state.regs[1],
                    r2: state.regs[2],
                    state,
                };

                return args.next()(args.pp, args.handlers, args.op, args.cp, args.state, args.r0, args.r1, args.r2);
            }
        }

        struct Args<'a> {
            pp: *const Op,
            handlers: Handlers,
            op: Op,
            cp: *const u8,
            state: &'a mut $state_ty,
            r0: u64,
            r1: u64,
            r2: u64,
        }

        impl Args<'_> {
            #[inline(always)]
            fn next(&mut self) -> Handler {
                // Safety: pp will point at valid op
                self.op = unsafe { *self.pp };
                // Safety: bytecode will always terminate or loop before the end of the buffer
                self.pp = unsafe { self.pp.add(1) };

                // Safety: all bytecode entries will point at a valid handler
                unsafe { *self.handlers.0.add(self.op.0 as usize & 0xff) }
            }

            fn get<const OPND: i32>(&mut self) -> &mut u64 {
                match OPND {
                    0 => &mut self.r0,
                    1 => &mut self.r1,
                    2 => &mut self.r2,
                    -1 => unsafe {
                        let reg = *self.cp;
                        self.cp = self.cp.add(1);
                        self.state.regs.get_unchecked_mut(reg as usize)
                    },
                    _ => unreachable!(),
                }
            }
        }
    };

    // Convert from tail-call form to Args struct
    ( @argfn
        fn $($name:ident)? $( < $( const $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+ >)?
            ( $args:ident : Args ) -> $ret:ty
            $($body:block)?
    ) => {
        fn $($name)? $(< $( const $lt $( : $clt $(+ $dlt )* )? ),+ >)? (
            pp: *const Op,
            handlers: Handlers,
            op: Op,
            cp: *const u8,
            state: &mut StateType,
            r0: u64,
            r1: u64,
            r2: u64
         ) -> $ret $( {

            #[allow(unused_mut)]
            let mut $args = Args { pp, handlers, op, cp, state, r0, r1, r2 };

            $body
         } )?
    };

    // Dispatch (converts from Args struct back to a tail call)
    ( @dispatch $args:ident ) => {
        become $args.next()($args.pp, $args.handlers, $args.op, $args.cp, $args.state, $args.r0, $args.r1, $args.r2)
    };

    // Munch binary ops - @op(Name($a: u64, $b: u64) -> u64 { ... })
    ( @munch($count:expr, $($parsed:tt)*)
        @op($opname:ident ($a:ident : $a_ty:tt, $b:ident : $b_ty:tt) -> $ret:tt $body:block)
        $( $tail:tt )*
    ) => {

        struct $opname;

        impl $opname {
            tailspin!(@argfn fn exec2<const A: i32, const B: i32, const DEST: i32>(args: Args) -> ExitCode {

                #[inline(always)]
                fn inner($a: u64, $b: u64) -> u64 $body

                let $a = *args.get::<A>();
                let $b = *args.get::<B>();

                *args.get::<DEST>() = inner($a, $b);

                tailspin!(@dispatch args)
            });

            //fn emit()
        }

        impl BytecodeOp for $opname {
            const OPCODE: Ops = Ops::$opname;
            fn get_handlers() -> Vec<Handler> {
                vec![
                    Self::exec2::<-1, -1, -1> as Handler,
                    Self::exec2::<-1, -1, 0> as Handler,
                    Self::exec2::<-1, -1, 1> as Handler,
                    Self::exec2::<-1, -1, 2> as Handler,

                    Self::exec2::<0, 1, -1> as Handler,
                    Self::exec2::<0, 1, 0> as Handler,
                    Self::exec2::<0, 1, 1> as Handler,
                    Self::exec2::<0, 1, 2> as Handler,

                    Self::exec2::<1, 2, -1> as Handler,
                    Self::exec2::<1, 2, 0> as Handler,
                    Self::exec2::<1, 2, 1> as Handler,
                    Self::exec2::<1, 2, 2> as Handler,

                    Self::exec2::<-1, 0, -1> as Handler,
                    Self::exec2::<-1, 0, 0> as Handler,
                    Self::exec2::<-1, 0, 1> as Handler,
                    Self::exec2::<-1, 0, 2> as Handler,

                    Self::exec2::<-1, 1, -1> as Handler,
                    Self::exec2::<-1, 1, 0> as Handler,
                    Self::exec2::<-1, 1, 1> as Handler,
                    Self::exec2::<-1, 1, 2> as Handler,

                    Self::exec2::<-1, 2, -1> as Handler,
                    Self::exec2::<-1, 2, 0> as Handler,
                    Self::exec2::<-1, 2, 1> as Handler,
                    Self::exec2::<-1, 2, 2> as Handler,
                ]
            }
        }

        tailspin!(@munch($count + 24usize, $($parsed)*
                $opname // Implicitly _RN_RN_RN
                [< $opname _Rn_Rn_R0 >]
                [< $opname _Rn_Rn_R1 >]
                [< $opname _Rn_Rn_R2 >]

                [< $opname _R0_R1_Rn >]
                [< $opname _R0_R1_R0 >]
                [< $opname _R0_R1_R1 >]
                [< $opname _R0_R1_R2 >]

                [< $opname _R1_R2_Rn >]
                [< $opname _R1_R2_R0 >]
                [< $opname _R1_R2_R1 >]
                [< $opname _R1_R2_R2 >]

                [< $opname _Rn_R0_Rn >]
                [< $opname _Rn_R0_R0 >]
                [< $opname _Rn_R0_R1 >]
                [< $opname _Rn_R0_R2 >]

                [< $opname _Rn_R1_Rn >]
                [< $opname _Rn_R1_R0 >]
                [< $opname _Rn_R1_R1 >]
                [< $opname _Rn_R1_R2 >]

                [< $opname _Rn_R2_Rn >]
                [< $opname _Rn_R2_R0 >]
                [< $opname _Rn_R2_R1 >]
                [< $opname _Rn_R2_R2 >]
            )
            $($tail)*
        );
    };

    // Munch binary, no result ops, with state - @op(Name($state: @state, $a: u64, $b: u64) { ... })
    ( @munch($count:expr, $($parsed:tt)*)
        @op($opname:ident ($state:ident : @state, $a:ident : $a_ty:tt, $b:ident : $b_ty:tt) $body:block)
        $( $tail:tt )*
    ) => {

        struct $opname;

        impl $opname {
            tailspin!(@argfn fn exec2<const A: i32, const B: i32>(args: Args) -> ExitCode {

                #[inline(always)]
                fn inner($state: &mut StateType, $a: u64, $b: u64) $body

                let $a = *args.get::<A>();
                let $b = *args.get::<B>();

                inner(args.state, $a, $b);

                tailspin!(@dispatch args)
            });
        }

        impl BytecodeOp for $opname {
            const OPCODE: Ops = Ops::$opname;
            fn get_handlers() -> Vec<Handler> {
                vec![
                    Self::exec2::<-1, -1> as Handler,
                    Self::exec2::<0, 1> as Handler,
                    Self::exec2::<0, 2> as Handler,
                    Self::exec2::<1, 0> as Handler,
                    Self::exec2::<1, 2> as Handler,
                    Self::exec2::<2, 0> as Handler,
                    Self::exec2::<2, 1> as Handler,
                    Self::exec2::<-1, 0> as Handler,
                    Self::exec2::<-1, 1> as Handler,
                    Self::exec2::<-1, 2> as Handler,
                    Self::exec2::<0, -1> as Handler,
                    Self::exec2::<1, -1> as Handler,
                    Self::exec2::<2, -1> as Handler,
                ]
            }
        }

        tailspin!(@munch($count + 13usize, $($parsed)*
                $opname // Implicitly _RN_RN
                [< $opname _R0_R1 >]
                [< $opname _R0_R2 >]
                [< $opname _R1_R0 >]
                [< $opname _R1_R2 >]
                [< $opname _R2_R0 >]
                [< $opname _R2_R1 >]

                [< $opname _Rn_R0 >]
                [< $opname _Rn_R1 >]
                [< $opname _Rn_R2 >]
                [< $opname _R0_Rn >]
                [< $opname _R1_Rn >]
                [< $opname _R2_Rn >]
            )
            $($tail)*
        );
    };

    // Munch unary op, with state - @op(Name($state:  @state, $a: u64) -> u64 { ... })
    ( @munch($count:expr, $($parsed:tt)*)
        @op($opname:ident ($state:ident : @state, $a:ident : $a_ty:tt) -> $ret:tt $body:block)
        $( $tail:tt )*
    ) => {
        struct $opname;

        impl $opname {
            tailspin!(@argfn fn exec2<const A: i32, const DEST: i32>(args: Args) -> ExitCode {

                #[inline(always)]
                fn inner($state: &mut StateType, $a: u64) -> u64 $body

                let $a = *args.get::<A>();

                *args.get::<DEST>() = inner(args.state, $a);

                tailspin!(@dispatch args)
            });
        }

        impl BytecodeOp for $opname {
            const OPCODE: Ops = Ops::$opname;
            fn get_handlers() -> Vec<Handler> {
                vec![
                    Self::exec2::<-1, -1> as Handler,
                    Self::exec2::<0, 1> as Handler,
                    Self::exec2::<0, 2> as Handler,
                    Self::exec2::<1, 0> as Handler,
                    Self::exec2::<1, 2> as Handler,
                    Self::exec2::<2, 0> as Handler,
                    Self::exec2::<2, 1> as Handler,
                    Self::exec2::<-1, 0> as Handler,
                    Self::exec2::<-1, 1> as Handler,
                    Self::exec2::<-1, 2> as Handler,
                    Self::exec2::<0, -1> as Handler,
                    Self::exec2::<1, -1> as Handler,
                    Self::exec2::<2, -1> as Handler,
                ]
            }
        }

        tailspin!(@munch($count + 13usize, $($parsed)*
                $opname // Implicitly _RN_RN
                [< $opname _R0_R1 >]
                [< $opname _R0_R2 >]
                [< $opname _R1_R0 >]
                [< $opname _R1_R2 >]
                [< $opname _R2_R0 >]
                [< $opname _R2_R1 >]

                [< $opname _Rn_R0 >]
                [< $opname _Rn_R1 >]
                [< $opname _Rn_R2 >]
                [< $opname _R0_Rn >]
                [< $opname _R1_Rn >]
                [< $opname _R2_Rn >]
            )
            $($tail)*
        );
    };

    // Munch @op(Name() -> @exit { ... })
    ( @munch($count:expr, $($parsed:tt)*)
        @op( $opname:ident() -> @exit $body:block )
        $($tail:tt)*
    ) => {
        struct $opname;

        impl $opname {
            tailspin!(@argfn fn exec(_args: Args) -> ExitCode {

                #[inline(always)]
                fn inner() -> ExitCode $body

                return inner();
            });
        }

        impl BytecodeOp for $opname {
            const OPCODE: Ops = Ops::$opname;
            fn get_handlers() -> Vec<Handler> {
                vec![ Self::exec as Handler ]
            }
        }

        tailspin!(@munch($count + 1usize, $($parsed)* $opname) $($tail)*);
    };

    // Terminal muncher. Actually build the enum with all the collected ops
    ( @munch($count:expr, $($parsed:tt)*)
        @enum($ops:ident)
    ) => {
        $crate::paste! {
            #[allow(non_camel_case_types)]
            #[allow(dead_code)]
            pub enum Ops {
                $( $parsed ),*
            }
        }

        static NUM_OPS: usize = $count;
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    struct State {
        regs: [u64; 32],
    }

    tailspin!(
        @state(State)

        @op( Add(a: u64, b: u64) -> u64 {a.wrapping_add(b)})
        //@op( Sub(a: u64, b: u64) -> u64 {a.wrapping_sub(b)})
        @op( Exit() -> @exit {
            println!("Exiting...");

            ExitCode(0)
        })
    );

    pub fn run() -> u64{
        let bytecode = [
            Op(Ops::Add as u32),
            Op(Ops::Exit as u32)
        ]; // add, exit
        let consts = [0u8, 1u8, 4u8];
        let interpreter = Interpreter::new();
        let pp = bytecode.as_ptr() as *const Op;
        let cp = consts.as_ptr() as *const u8;

        println!("Starting tailspin...");
        println!("pp: {:x}, cp: {:x}", pp as usize, cp as usize);

        let mut state = State {
            regs: [0; 32],
        };
        state.regs[0] = 5;
        state.regs[1] = 2;

        let exit = unsafe { interpreter.run(pp, cp, &mut state) };

        println!("Exit code: {:?}", exit.0);
        println!("R0: {:?}", state.regs[0]);
        println!("R1: {:?}", state.regs[1]);

        state.regs[4]
    }

    #[test]
    fn test_tailcall() {
        fn tail_recursive(n: u64, acc: u64) -> u64 {

            println!("n: {n}, stack_var: {:?}", psm::stack_pointer());
            if n == 0 {
                acc
            } else {
                become tail_recursive(n - 1, n * acc)
            }
        }

        assert_eq!(tail_recursive(5, 1), 120);
    }

    #[test]
    fn tailspin() {
        let result = run();
        assert_eq!(result, 7);
    }

}