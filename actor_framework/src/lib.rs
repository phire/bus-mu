// FIXME: can we do it without this?
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

#![feature(associated_type_defaults)]
#![feature(ptr_metadata)]
#![feature(arbitrary_self_types)]
#![feature(dispatch_from_dyn)]

mod actor_box;
mod addr;
mod channel;
mod message_packet;
mod scheduler;
mod time;
mod enum_map;
mod object_map;
mod named;

pub use actor_box::{ActorBox, ActorBoxBase, AsBase};
pub use addr::Addr;
pub use channel::Channel;
pub use named::{Named, MakeNamed};
pub use named_derive::Named;
use object_map::ObjectStore;
pub use time::Time;
pub use message_packet::{MessagePacket, MessagePacketProxy};
pub use message_packet::{Outbox, OutboxSend};
pub use scheduler::{Scheduler, SchedulerResult};
pub use enum_map::EnumMap;

pub trait Actor<ActorNames> : Named<ActorNames>
where
    ActorNames: MakeNamed,
    Self::OutboxType: Outbox<ActorNames>,
    //[(); ActorNames::COUNT]:
{
    type OutboxType;

    //fn get_message<'a>(self: &mut ActorBox<ActorNames, Self>) -> Pin<&'a mut MessagePacketProxy<ActorNames>>;
    fn message_delivered(&mut self, _outbox: &mut Self::OutboxType, _time: Time) { }
}

pub trait Handler<ActorNames, M>
where
    ActorNames: MakeNamed,
{
    fn recv(&mut self, outbox: &mut Self::OutboxType, message: M, time: Time, limit: Time) -> SchedulerResult
    where
        Self: Actor<ActorNames> + Sized
    ;
}

pub trait Sender<ActorNames, Message> : Actor<ActorNames>
where
    ActorNames: MakeNamed,
{
    // fn take(map: &mut ObjectStore<ActorNames>) -> (Time, Message);
    fn as_mut<'b, 'c>(map: &'b mut ObjectStore<ActorNames>) -> Option<&'c mut MessagePacket<ActorNames, Message>>
    where
        Self: Sized + 'c,
        &'c mut ActorBox<ActorNames, Self>: From<&'b mut <ActorNames as MakeNamed>::StorageType>,
    //where
        //T: Actor<ActorNames> + 'b,
        //&'b mut ActorBox<ActorNames, T>: From<&'b mut <ActorNames as MakeNamed>::StorageType>
    ;
    fn delivered<'b, 'c>(map: &'b mut ObjectStore<ActorNames>, time: Time)
    where 'b: 'c,
        Self: Sized + 'c,
       &'c mut ActorBox<ActorNames, Self>: From<&'b mut <ActorNames as MakeNamed>::StorageType>
    ;
}

impl<'a, 'd, 'e, ActorNames, Message, T> Sender<ActorNames, Message> for T
where
    ActorNames: MakeNamed, //<StorageType = ActorBox<ActorNames, Self>>,
    T: Actor<ActorNames> + 'd,
    //<ActorNames as MakeNamed>::Base: Actor<ActorNames>,
    //&'d mut ActorBox<ActorNames, T>: From<&'e mut <ActorNames as MakeNamed>::StorageType>,
    <Self as Actor<ActorNames>>::OutboxType: Outbox<ActorNames> + OutboxSend<ActorNames, Message>,
{
    // fn take(map: &mut ObjectStore<ActorNames>) -> (Time, Message) {
    //     let actor: &mut ActorBox<ActorNames, T> = map.get::<Self>();
    //     actor.outbox.cancel()
    // }
    fn as_mut<'b, 'c>(map: &'b mut ObjectStore<ActorNames>) -> Option<&'c mut MessagePacket<ActorNames, Message>>
    where
        Self: 'c,
        &'c mut ActorBox<ActorNames, T>: From<&'b mut <ActorNames as MakeNamed>::StorageType>,
    {
        let actor = map.get::<Self>();
        actor.outbox.as_packet()
    }
    fn delivered<'b, 'c>(map: &'b mut ObjectStore<ActorNames>, time: Time)
    where
        'b: 'c,
        Self: 'c,
        &'c mut ActorBox<ActorNames, T>: From<&'b mut <ActorNames as MakeNamed>::StorageType>,
     {
        let actor = map.get::<Self>();
        actor.actor.message_delivered(&mut actor.outbox, time);
    }
}

pub trait Receiver<'o, ActorNames, Message> : Actor<ActorNames> + Handler<ActorNames, Message>
where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::StorageType: Sized + 'a,
    Self: Sized,
    Self: 'b,
    'a: 'b,
    //&'b mut ActorBox<ActorNames, Self>: From<&'a mut <ActorNames as MakeNamed>::StorageType>,
{
    fn receive(map: &'a mut ObjectStore<ActorNames>, message: Message, time: Time, limit: Time) -> SchedulerResult
    where
        // 'd: 'c,
        // Self: Sized + 'c,
        //&'c mut ActorBox<ActorNames, Self>: From<&'d mut <ActorNames as MakeNamed>::StorageType>,
        &'b mut ActorBox<ActorNames, Self>: From<&'a mut <ActorNames as MakeNamed>::StorageType>,
        ;
}

impl<'a, 'b, ActorNames, Message, T> Receiver<'a, 'b, ActorNames, Message> for T
where
    ActorNames: MakeNamed,
    <ActorNames as MakeNamed>::StorageType: Sized + 'a,
    T: Actor<ActorNames> + Handler<ActorNames, Message> + Sized + 'b,
    <T as Actor<ActorNames>>::OutboxType: Outbox<ActorNames>,
    //&'b mut ActorBox<ActorNames, Self>: From<&'a mut <ActorNames as MakeNamed>::StorageType>,
    'a: 'b,
    //<ActorNames as MakeNamed>::Base: Actor<ActorNames>,

{
    fn receive(map: &'a mut ObjectStore<ActorNames>, message: Message, time: Time, limit: Time) -> SchedulerResult
    where
        &'b mut ActorBox<ActorNames, Self>: From<&'a mut <ActorNames as MakeNamed>::StorageType>,
        // 'd: 'c,
        // Self: Sized + 'c,
        //&'c mut ActorBox<ActorNames, Self>: From<&'d mut <ActorNames as MakeNamed>::StorageType>,
    {
        let receiver = map.get::<Self>();
        receiver.actor.recv(&mut receiver.outbox, message, time, limit)
    }
}