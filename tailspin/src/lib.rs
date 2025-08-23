#![allow(incomplete_features)]
#![feature(explicit_tail_calls)]

pub use paste::paste;

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