
#![feature(associated_type_defaults)]

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

use std::{sync::mpsc, marker::PhantomData};

pub use actor_box::{ActorBox, ActorBoxBase, AsBase};
pub use addr::Addr;
pub use channel::Channel;
use common::{UpdateMessage, ControlMessage, State};
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

pub struct ActorFrameworkCore<ActorNames> where ActorNames: MakeNamed {
    actor_names: PhantomData<ActorNames>,
    name: &'static str,
}

impl <ActorNames> ActorFrameworkCore<ActorNames> where ActorNames: MakeNamed {
    pub fn new() -> ActorFrameworkCore<ActorNames> {
        ActorFrameworkCore {
            actor_names: PhantomData,
            name: "Unnamed ActorFrameworkCore",
        }
    }
    pub fn set_name(&mut self, name: &'static str) {
        self.name = name;
    }
}

impl<ActorNames> common::Core for ActorFrameworkCore<ActorNames>
where
    ActorNames: MakeNamed,
{
    fn name(&self) -> &'static str {
        self.name
    }

    fn create(&self) -> Result<common::CoreCommunication, anyhow::Error>
    {
        let (control_tx, control_rx) = mpsc::sync_channel::<ControlMessage>(1);
        let (updates_tx, updates_rx) = mpsc::sync_channel::<UpdateMessage>(10);

        let join =
            std::thread::spawn(move || -> Result<(), anyhow::Error> {

                let result =std::panic::catch_unwind(|| -> Result<(), anyhow::Error> {
                    let mut scheduler = Scheduler::<ActorNames>::new();

                    loop {
                        match control_rx.recv()? {
                            ControlMessage::MoveTo(State::Run) => {
                                updates_tx.send(UpdateMessage::MovedTo(State::Run))?;
                                if scheduler.run(&control_rx, &updates_tx)? {
                                    return Ok(()); // Exit
                                }
                                updates_tx.send(UpdateMessage::MovedTo(State::Pause))?;
                            }
                            #[cfg(feature = "ui")]
                            ControlMessage::DoUi(_ctx) => {
                                todo!("Ui while paused")
                            }
                            _ => { unreachable!() }
                        }
                    }
                });
                result.map_err(|_| anyhow::anyhow!("core paniced"))?
            });

        Ok(common::CoreCommunication {
            control: control_tx,
            update: updates_rx,
            join
        })
    }
}

