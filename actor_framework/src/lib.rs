mod actor_box;
mod addr;
mod channel;
mod endpoint;
mod enum_map;
mod message_packet;
mod named;
mod object_map;
mod outbox;
mod scheduler;
mod time;
mod time_queue;

use std::sync::mpsc;

pub use actor_box::{ActorBox, ActorBoxBase, AsBase};
pub use addr::Addr;
pub use channel::Channel;
use common::{ControlMessage, UpdateMessage};
pub use endpoint::Endpoint;
pub use enum_map::EnumMap;
pub use message_packet::{MessagePacket, MessagePacketProxy};
pub use named::{MakeNamed, Named};
pub use named_derive::Named;
pub use outbox::{Outbox, OutboxSend};
pub use scheduler::{Scheduler, SchedulerResult};
pub use time::Time;
pub use time_queue::TimeQueue;

pub trait Actor<ActorNames>: Named<ActorNames>
where
    ActorNames: MakeNamed,
    Self::OutboxType: Outbox<ActorNames>,
{
    type OutboxType;

    /// `delivering` is called just before the scheduler delivers a message
    ///
    /// The message has already been removed from the outbox, allowing the actor to send
    /// a new message, or restore the a previous message that was interrupted.
    #[inline(always)]
    fn delivering<Message>(&mut self, outbox: &mut Self::OutboxType, message: &Message, time: Time)
    where
        Message: 'static,
    {
        // Default implementation: do nothing
        let _ = (outbox, message, time);
    }
}

pub trait ActorCreate<ActorNames>: Actor<ActorNames>
where
    ActorNames: MakeNamed,
    Self::OutboxType: Outbox<ActorNames>,
{
    fn new(outbox: &mut Self::OutboxType, time: Time) -> Self;
}

impl<ActorNames, T> ActorCreate<ActorNames> for T
where
    ActorNames: MakeNamed,
    T: Actor<ActorNames> + Default,
{
    fn new(_outbox: &mut Self::OutboxType, _time: Time) -> T {
        T::default()
    }
}

pub trait Handler<ActorNames, M>: Actor<ActorNames>
where
    ActorNames: MakeNamed,
{
    fn recv(
        &mut self,
        outbox: &mut Self::OutboxType,
        message: M,
        time: Time,
        limit: Time,
    ) -> SchedulerResult
    where
        Self: Actor<ActorNames> + Sized;
}

pub struct Instance<ActorNames>
where
    ActorNames: MakeNamed,
{
    scheduler: Scheduler<ActorNames>,
}

impl<ActorNames> Instance<ActorNames>
where
    ActorNames: MakeNamed,
{
    pub fn new() -> Instance<ActorNames> {
        Instance {
            scheduler: Scheduler::<ActorNames>::new(),
        }
    }

    pub fn actor<ActorType>(&mut self) -> &mut ActorType
    where
        ActorType: Actor<ActorNames>,
    {
        self.scheduler.get::<ActorType>()
    }
}

impl<ActorNames> common::Instance for Instance<ActorNames>
where
    ActorNames: MakeNamed,
{
    fn run(
        &mut self,
        control_rx: &mpsc::Receiver<ControlMessage>,
        update: mpsc::SyncSender<UpdateMessage>,
    ) -> Result<(), anyhow::Error> {
        self.scheduler.run(control_rx, update)
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
