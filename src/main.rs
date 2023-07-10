#![feature(generic_const_exprs)]

pub mod n64;
pub mod actor;
pub mod object_map;

use named_derive::Named;
use object_map::{Named, MakeNamed};

#[derive(Default)]
struct ThingA;
#[derive(Default)]
struct ThingB;
#[derive(Default)]
struct ThingC;

#[derive(Named, PartialEq, Eq, Copy, Clone, Debug)]
enum Test {
    #[named(class(ThingA))]
    A,
    #[named(class(ThingB))]
    B,
     #[named(class(ThingC))]
    C,
}

fn main() {
    //n64::vr4300::test();
    println!("Initializing Scheduler");
    let mut scheduler = actor::Scheduler::<Test>::new();
    println!("Starting Scheduler");
    scheduler.run();

}
