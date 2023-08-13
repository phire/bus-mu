
mod actor_box;
mod addr;
mod channel;
mod endpoint;
mod message_packet;
mod scheduler;
mod time;
mod enum_map;
mod object_map;
mod named;
mod outbox;

use std::sync::mpsc;

pub use actor_box::{ActorBox, ActorBoxBase, AsBase};
pub use addr::Addr;
pub use channel::Channel;
use common::{UpdateMessage, ControlMessage};
pub use endpoint::Endpoint;
pub use named::{Named, MakeNamed};
pub use named_derive::Named;
pub use time::Time;
pub use message_packet::{MessagePacket, MessagePacketProxy};
pub use outbox::{Outbox, OutboxSend};
pub use scheduler::{Scheduler, SchedulerResult};
pub use enum_map::EnumMap;

pub trait Actor<ActorNames> : Named<ActorNames>
where
    ActorNames: MakeNamed,
    Self::OutboxType: Outbox<ActorNames>,
{
    type OutboxType;

    /// `message_delivered` is called immediately after the scheduler delivered this actor's previous message.
    ///
    /// Useful for actors that need to send multiple messages at once, or restore message
    /// that was previously interrupted
    #[inline(always)]
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

pub struct Instance<ActorNames> where ActorNames: MakeNamed {
    scheduler: Scheduler<ActorNames>,
}

impl<ActorNames> Instance<ActorNames> where ActorNames: MakeNamed {
    pub fn new() -> Instance<ActorNames> {

        Instance {
            scheduler: Scheduler::<ActorNames>::new(),
        }
    }
}

impl<ActorNames> common::Instance for Instance<ActorNames>
where
    ActorNames: MakeNamed,
{
    fn run(&mut self,
        control_rx: &mpsc::Receiver<ControlMessage>,
        update: mpsc::SyncSender<UpdateMessage>
    ) -> Result<(), anyhow::Error> {
        self.scheduler.run(control_rx, update)
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}


