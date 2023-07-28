// FIXME: can we do it without this?
#![feature(generic_const_exprs)]

mod messaging;
mod scheduler;
mod time;
mod object_map;

pub use object_map::{Named, MakeNamed};
pub use named_derive::Named;
pub use time::Time;
pub use messaging::{MessagePacket, Addr, Channel};
pub use scheduler::Scheduler;
pub use object_map::EnumMap;

pub trait Actor<ActorNames> : Named<ActorNames> {
    fn advance(&mut self, limit: Time) -> MessagePacket<ActorNames>;
    fn advance_to(&mut self, target: Time);

    /// This actor guarantees that it will not send any messages before this time
    fn horizon(&mut self) -> Time;
}

pub trait Handler<M, ActorNames> where Self: Named<ActorNames> {
    fn recv(&mut self, time: Time, message: M) -> MessagePacket<ActorNames>;
}
