#![feature(explicit_tail_calls)]


#[derive(Copy, Clone)]
struct Op(u32);
struct State {
    r0: u64,
    r1: u64,
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
        fn $name:ident $( < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+ >)? ($args:ident: Args) -> ExitCode $body:block
    ) => {
        fn $name$ (< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)?
        (pp: *const Op, handlers: Handlers, op: Op, cp: *const u8, state: &mut State, r0: u64, r1: u64, r2: u64) -> ExitCode
        {
            #[allow(unused_mut)]
            let mut $args = Args { pp, handlers, op, cp, state, r0, r1, r2 };

            $body
        }
    };

    (
        fn $name:ident $( < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+ >)? ($args:ident: Args) $body:block
    ) => {
        handler_fn! {
            fn $name$ (< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? ($args: Args) -> ExitCode {
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

#[inline(never)]
fn start(pp: *const Op, cp: *const u8, state: &mut State) -> ExitCode {
    let r0 = state.r0;
    let r1 = state.r1;
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

struct TwoReg;
impl Mode for TwoReg {
    fn exec2<F: FnOnce(u64, u64) -> u64>(f: F, args: &mut Args) {
        args.r0 = f(args.r0, args.r2);
    }
}

#[derive(Copy, Clone)]
struct Handlers(*const Handler);

macro_rules! wrap_op {
    ( // Two arg
        $name:ident($a:ident: u64, $b:ident: u64) -> u64 $body:block
    ) => {
        handler_fn!{
            fn $name<M: Mode>(args: Args) {
                #[inline(always)]
                fn inner($a: u64, $b: u64) -> u64 $body

                M::exec2(inner, &mut args);
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

                args.state.r0 = args.r0;
                args.state.r1 = args.r1;

                return exitcode;
            }
        }
    };
}

wrap_op!( op_add(a: u64, b: u64) -> u64 {
    a + b
});

wrap_op!( op_sub(a: u64, b: u64) -> u64 {
    a - b
});

wrap_op!( op_exit() -> ExitCode {
    println!("Exiting...");

    ExitCode(0)
});

const OPS: [Handler; 4] = [
    op_add::<TwoReg>,
    op_sub::<TwoReg>,
    op_exit,
    op_exit,
];

pub fn run() -> u64{
    let bytecode = [Op(0), Op(3)]; // add, exit
    let consts = [0u8, 0u8];
    let pp = bytecode.as_ptr() as *const Op;
    let cp = consts.as_ptr() as *const u8;

    println!("Starting tailspin...");

    let mut state = State {
        r0: 2,
        r1: 5,
    };

    let exit = start(pp, cp, &mut state);

    println!("Exit code: {:?}", exit.0);
    println!("R0: {:?}", state.r0);
    println!("R1: {:?}", state.r1);

    state.r0
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