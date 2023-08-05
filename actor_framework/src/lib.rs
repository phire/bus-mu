// FIXME: can we do it without this?
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

#![feature(associated_type_defaults)]
#![feature(ptr_metadata)]

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
pub use scheduler::{Scheduler, SchedulerResult};
pub use enum_map::EnumMap;

use std::pin::Pin;

pub trait Actor<ActorNames> : Named<ActorNames>
where
    ActorNames: MakeNamed,
    [(); ActorNames::COUNT]:
{
    fn get_message<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut MessagePacketProxy<ActorNames>>;
    fn message_delivered(self: Pin<&mut Self>, time: Time);
}

pub trait Handler<M>
{
    fn recv(&mut self, message: M, time: Time, limit: Time) -> SchedulerResult;
}
