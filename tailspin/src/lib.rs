#![feature(explicit_tail_calls)]

#[derive(Copy, Clone)]
struct Op(u32);
struct State {
    regs: [u64; 32],
}

struct ExitCode(u32);

struct Args<'a>{
    pp: *const Op,
    handlers: Handlers,
    op: Op,
    cp: *const u8,
    state: &'a mut State,
    r0: u64,
    r1: u64,
    r2: u64,
}


type Handler = fn(pp: *const Op, handlers: Handlers, op: Op, cp: *const u8, state: &mut State, r0: u64, r1: u64, r2: u64) -> ExitCode;

macro_rules! handler_fn {
    // This nested macro is here to minimize the number of places we need to update the handler function prototype.
    // Once we have a function prototype locked in, it might be wise to just duplicate it everywhere.
    (
        fn $name:ident $( < $( const $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+ >)? ($args:ident: Args) -> ExitCode $body:block
    ) => {
        fn $name$ (< $( const $lt $( : $clt $(+ $dlt )* )? ),+ >)?
        (pp: *const Op, handlers: Handlers, op: Op, cp: *const u8, state: &mut State, r0: u64, r1: u64, r2: u64) -> ExitCode
        {
            #[allow(unused_mut)]
            let mut $args = Args { pp, handlers, op, cp, state, r0, r1, r2 };

            $body
        }
    };

    (
        fn $name:ident $( < $( const $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+ >)? ($args:ident: Args) $body:block
    ) => {
        handler_fn! {
            fn $name$ (< $( const $lt $( : $clt $(+ $dlt )* )? ),+ >)? ($args: Args) -> ExitCode {
                $body

                // Safety: bytecode will always terminate or loop before the end of the buffer
                unsafe {
                    $args.op = *$args.pp;
                    $args.pp = $args.pp.add(1);
                }

                // Safety: all bytecode entries will point at a valid handler
                let handler = unsafe { *$args.handlers.0.add($args.op.0 as usize & 0xff) };

                become handler($args.pp, $args.handlers, $args.op, $args.cp, $args.state, $args.r0, $args.r1, $args.r2);
            }
        }
    };
}


impl Args<'_> {
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


#[inline(never)]
fn start(pp: *const Op, cp: *const u8, state: &mut State) -> ExitCode {
    let r0 = state.regs[0];
    let r1 = state.regs[1];
    let r2 = 0;

    let handlers = Handlers(OPS.as_ptr());
    let op = Op(0); // Dummy op

    // Use the handler_fn macro to instance the dispatch code
    handler_fn! { fn start_inner(args: Args) {} }
    return start_inner(pp, handlers, op, cp, state, r0, r1, r2);
}


trait Mode {
    fn exec2<F: FnOnce(u64, u64) -> u64>(f: F, args: &mut Args);
}



#[derive(Copy, Clone)]
struct Handlers(*const Handler);

macro_rules! wrap_op {
    ( // Two arg
        $name:ident($a:ident: u64, $b:ident: u64) -> u64 $body:block
    ) => {
        handler_fn!{
            fn $name<const A: i32, const B: i32, const DEST: i32>(args: Args) {
                #[inline(always)]
                fn inner($a: u64, $b: u64) -> u64 $body

                let a = *args.get::<A>();
                let b = *args.get::<B>();
                *args.get::<DEST>() = inner(a, b);

                //M::exec2(inner, &mut args);
            }
        }
    };

    ( // Access args
        $name:ident($args:ident: &mut Args) $body:block
    ) => {
        handler_fn!{
            fn $name($args: Args) {

                $body
            }
        }
    };

    // Always Terminate
    ($name:ident() -> ExitCode $body:block) => {
        handler_fn!{
            fn $name(args: Args) -> ExitCode {
                let exitcode = $body;

                args.state.regs[0] = args.r0;
                args.state.regs[1] = args.r1;
                args.state.regs[2] = args.r2;

                return exitcode;
            }
        }
    };
}

wrap_op!( op_add(a: u64, b: u64) -> u64 {
    u64::wrapping_add(a, b)
});

wrap_op!( op_sub(a: u64, b: u64) -> u64 {
    u64::wrapping_sub(a, b)
});

wrap_op!( op_exit() -> ExitCode {
    println!("Exiting...");

    ExitCode(0)
});

struct TwoReg;
impl Mode for TwoReg {
    fn exec2<F: FnOnce(u64, u64) -> u64>(f: F, args: &mut Args) {
        args.r0 = f(args.r0, args.r2);
    }
}

const OPS: [Handler; 4] = [
    op_add::<-1, -1, -1>,
    op_add::<-1, -1, 0>,
    op_sub::<-1, -1, -1>,
    op_exit,
];

pub fn run() -> u64{
    let bytecode = [Op(0), Op(3)]; // add, exit
    let consts = [0u8, 1u8, 4u8];
    let pp = bytecode.as_ptr() as *const Op;
    let cp = consts.as_ptr() as *const u8;

    println!("Starting tailspin...");

    let mut state = State {
        regs: [0; 32],
    };
    state.regs[0] = 5;
    state.regs[1] = 2;

    let exit = start(pp, cp, &mut state);

    println!("Exit code: {:?}", exit.0);
    println!("R0: {:?}", state.regs[0]);
    println!("R1: {:?}", state.regs[1]);

    state.regs[4]
}

#[cfg(test)]
mod tests {
    use super::*;

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