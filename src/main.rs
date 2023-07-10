#![feature(variant_count)]
#![feature(generic_const_exprs)]

pub mod n64;
pub mod actor;
pub mod object_map;

fn main() {
    //n64::vr4300::test();
    let mut scheduler = actor::Scheduler::new();
    scheduler.run();

}
