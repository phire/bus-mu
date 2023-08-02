// FIXME: can we do it without this?
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]


pub mod actors;
pub mod vr4300;

pub mod cic;
pub mod pif;

pub use actors::N64Actors;

pub fn new() -> actor_framework::Scheduler<N64Actors> {
    actor_framework::Scheduler::new()
}