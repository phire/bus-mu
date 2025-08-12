#![feature(explicit_tail_calls)]


#[derive(Copy, Clone)]
struct Op(u32);
struct State {
    r0: u64,
    r1: u64,
}

struct ExitCode(u32);

trait Mode {
    fn exec2<F: FnOnce(u64, u64) -> u64>(f: F, op: Op, cp: *const u8, state: &mut State, r0: u64, r1: u64) -> (*const u8, u64, u64);
}

struct TwoReg;
impl Mode for TwoReg {
    fn exec2<F: FnOnce(u64, u64) -> u64>(f: F, _op: Op, cp: *const u8, _state: &mut State, r0: u64, r1: u64) -> (*const u8, u64, u64) {
        let r0 = f(r0, r1);
        (cp, r0, r1)
    }
}


type Handler = fn(op: Op, pp: *const Op, cp: *const u8, state: &mut State, r0: u64, r1: u64, handlers: Handlers) -> ExitCode;

#[derive(Copy, Clone)]
struct Handlers(*const Handler);

#[inline(always)]
fn next_op(pp: *const Op, handlers: Handlers) -> (Op, Handler, *const Op) {
    // Safety: bytecode will always terminate or loop before the end of the buffer
    let (op, pp) = unsafe { (*pp, pp.add(1)) };

    // Safety: all bytecode entries will point at a valid handler
    let handler = unsafe { *handlers.0.add(op.0 as usize) };

    (op, handler, pp)
}

#[inline(always)]
fn dispatch(op: Op, pp: *const Op, cp: *const u8, state: &mut State, r0: u64, r1: u64, handlers: Handlers) -> ExitCode {
    let _ = op;

    // Safety: bytecode will always terminate or loop before the end of the buffer
    let (op, pp) = unsafe { (*pp, pp.add(1)) };

    // Safety: all bytecode entries will point at a valid handler
    let handler = unsafe { *handlers.0.add(op.0 as usize & 0xff) };

    become handler(op, pp, cp, state, r0, r1, handlers);
}

#[inline(never)]
fn start(_: Op, pp: *const Op, cp: *const u8, state: &mut State) -> ExitCode {
    let r0 = state.r0;
    let r1 = state.r1;

    let handlers = Handlers(OPS.as_ptr());

    let (op, handler, pp) = next_op(pp, handlers);
    return handler(op, pp, cp, state, r0, r1, handlers);
}

macro_rules! wrap_op {

    // Two arg
    (
        $name:ident($a:ident: u64, $b:ident: u64) -> u64 {
            $($body:tt)*
        }
    ) => {
        #[inline(never)]
        fn $name<M: Mode>(op: Op, pp: *const Op, cp: *const u8, state: &mut State, r0: u64, r1: u64, handlers: Handlers) -> ExitCode {
            #[inline(always)]
            fn inner($a: u64, $b: u64) -> u64 {
                $($body)*
            }

            let (cp, r0, r1) = M::exec2(inner, op, cp, state, r0, r1);

            become dispatch(op, pp, cp, state, r0, r1, handlers);
        }
    };

    // Always Terminate
    ($name:ident() -> ExitCode { $($body:tt)* }) => {
        #[inline(never)]
        fn $name(_: Op, _: *const Op, _: *const u8, state: &mut State, r0: u64, r1: u64, _handlers: Handlers) -> ExitCode {

            let exitcode = {
                $($body)*
            };

            state.r0 = r0;
            state.r1 = r1;

            return exitcode;
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

pub fn run() {
    let bytecode = [Op(0), Op(3)]; // add, exit
    let consts = [0u8, 0u8];
    let pp = bytecode.as_ptr() as *const Op;
    let cp = consts.as_ptr() as *const u8;


    println!("Starting tailspin...");

    let mut state = State {
        r0: 2,
        r1: 5,
    };

    let exit = start(Op(0), pp, cp, &mut state);

    println!("Exit code: {:?}", exit.0);
    println!("R0: {:?}", state.r0);
    println!("R1: {:?}", state.r1);
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
        run();
    }

}