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
pub use message_packet::{MessagePacket, MessagePacketProxy};
pub use message_packet::{Outbox, OutboxSend};
pub use scheduler::Scheduler;
pub use enum_map::EnumMap;

pub trait Actor<ActorNames> : Named<ActorNames>
where
    ActorNames: MakeNamed,
    [(); ActorNames::COUNT]:
{
    fn get_message(&mut self) -> &mut MessagePacketProxy<ActorNames>;
    fn message_delivered(&mut self, time: &Time);
}

pub trait Handler<M> where {
    fn recv(&mut self, message: M, time: Time, limit: Time);
}
