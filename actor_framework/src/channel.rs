use std::marker::PhantomData;

use crate::{MakeNamed, Actor, Handler};

/// Allows registering a channel between a Sender and Receiver for a given Message type.
///
/// Channel uses static dispatch internally, to make delivering messages as fast as a direct send.
/// If the Sender is dynamic compile time, use `Endpoint` instead.
#[derive(Copy)]
pub struct Channel<ActorNames, Sender, Message>
    where
        ActorNames: MakeNamed,
        Sender: Actor<ActorNames>,
{
    pub(super) execute_fn: crate::scheduler::ExecuteFn<ActorNames>,
    receiver: ActorNames,
    message_type: PhantomData<Message>,
    sender: PhantomData<Sender>,
}

impl<ActorNames, Sender, Message> Channel<ActorNames, Sender, Message>
where
    ActorNames: MakeNamed,
    Sender: Actor<ActorNames>,
{
    pub fn new<Receiver>() -> Channel<ActorNames, Sender, Message>
    where
        Receiver : Handler<ActorNames, Message> + Actor<ActorNames>,
        <Sender as Actor<ActorNames>>::OutboxType: crate::OutboxSend<ActorNames, Message>,
        Message: 'static, // for TypeId
    {
        Channel {
            // Safety: It is essential that we instantiate the correct execute_fn
            //         template here. It relies on this function for type checking
            execute_fn: crate::scheduler::direct_execute::<ActorNames, Sender, Receiver, Message>,
            receiver: Receiver::name(),
            message_type: PhantomData,
            sender: PhantomData,
        }
    }

    pub fn receiver(&self) -> ActorNames {
        self.receiver
    }
}

impl<ActorNames, Sender, Message> Clone for Channel<ActorNames, Sender, Message>
    where ActorNames: MakeNamed,
        Sender: Actor<ActorNames>,
{
    fn clone(&self) -> Self {
        Channel {
            execute_fn: self.execute_fn,
            receiver: self.receiver,
            message_type: PhantomData,
            sender: PhantomData,
        }
    }
}