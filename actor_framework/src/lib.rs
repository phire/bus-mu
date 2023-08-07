
#![feature(associated_type_defaults)]

mod actor_box;
mod addr;
mod channel;
mod message_packet;
mod scheduler;
mod time;
//mod enum_map;
mod object_map;
mod named;

pub use actor_box::{ActorBox, ActorBoxBase, AsBase};
pub use addr::Addr;
pub use channel::Channel;
pub use named::{Named, MakeNamed};
pub use named_derive::Named;
pub use time::Time;
pub use message_packet::{MessagePacket, MessagePacketProxy};
pub use message_packet::{Outbox, OutboxSend};
pub use scheduler::{Scheduler, SchedulerResult};
//pub use enum_map::EnumMap;

pub trait Actor<ActorNames> : Named<ActorNames>
where
    ActorNames: MakeNamed,
    Self::OutboxType: Outbox<ActorNames>,
{
    type OutboxType;

    fn message_delivered(&mut self, _outbox: &mut Self::OutboxType, _time: Time) { }
}

pub trait ActorCreate<ActorNames> : Actor<ActorNames>
where
    ActorNames: MakeNamed,
    Self::OutboxType: Outbox<ActorNames>,
{
    fn new(outbox: &mut Self::OutboxType, time: Time) -> Self;
}

impl<ActorNames, T> ActorCreate<ActorNames> for T
 where ActorNames: MakeNamed,
       T: Actor<ActorNames> + Default,
 {
    fn new(_outbox: &mut Self::OutboxType, _time: Time) -> T {
        T::default()
    }
 }

pub trait Handler<ActorNames, M> : Actor<ActorNames>
where
    ActorNames: MakeNamed,
{
    fn recv(&mut self, outbox: &mut Self::OutboxType, message: M, time: Time, limit: Time) -> SchedulerResult
    where
        Self: Actor<ActorNames> + Sized
    ;
}
