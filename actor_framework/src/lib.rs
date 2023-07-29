// FIXME: can we do it without this?
#![feature(generic_const_exprs)]

mod addr;
mod channel;
mod message_packet;
mod scheduler;
mod time;
mod enum_map;
mod object_map;
mod named;

pub use addr::Addr;
pub use channel::Channel;
pub use named::{Named, MakeNamed};
pub use named_derive::Named;
pub use time::Time;
pub use message_packet::MessagePacket;
pub use scheduler::Scheduler;
pub use enum_map::EnumMap;

pub trait Actor<ActorNames> : Named<ActorNames> {
    fn advance(&mut self, limit: Time) -> MessagePacket<ActorNames>;
    fn advance_to(&mut self, target: Time);

    /// This actor guarantees that it will not send any messages before this time
    fn horizon(&mut self) -> Time;
}

pub trait Handler<M, ActorNames> where Self: Named<ActorNames> {
    fn recv(&mut self, time: Time, message: M) -> MessagePacket<ActorNames>;
}
