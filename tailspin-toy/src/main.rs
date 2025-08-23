#![allow(incomplete_features)]
#![feature(explicit_tail_calls)]

use tailspin::tailspin;

struct State {
   regs: [u64; 32],
}

tailspin! {
    @state(State)

    @op( Add(a: u64, b: u64) -> u64 {a.wrapping_add(b)})
    @op( Exit() -> @exit {
        println!("Exiting...");

        ExitCode(0)
    })
}

fn main() {

}