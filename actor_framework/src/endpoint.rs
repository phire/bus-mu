use crate::{MakeNamed, Handler, Addr, Actor};

/// An Endpoint is half of a `Channel`.
/// The Receiver and Message type is known at compile time but the Sender is dynamically dispatched.
///
/// Not only does this extra level dynamic dispatch add some overhead, but it prevents some inlining
/// based optimizations in `Scheduler`.
///
/// So `Channel` should be preferred if it's possible to know both the Sender and Receiver at compile time,
/// as it uses static dispatch.
#[derive(Copy)]
pub struct Endpoint<ActorNames, Message>
    where
        ActorNames: MakeNamed,
        Message: 'static
{
    pub(super) endpoint_fn: crate::scheduler::EndpointFn<ActorNames, Message>,
}

impl<ActorNames, Message> Endpoint<ActorNames, Message>
where
    ActorNames: MakeNamed
{
    pub fn new<Receiver>() -> Endpoint<ActorNames, Message>
    where
        Receiver : Handler<ActorNames, Message> + Actor<ActorNames>,
    {
        Endpoint {
            endpoint_fn: crate::scheduler::receive_for_endpoint::<ActorNames, Receiver, Message>,
        }
    }
}

impl<ActorNames, Message> Clone for Endpoint<ActorNames, Message>
    where ActorNames: MakeNamed,
        //<ActorNames as MakeNamed>::Base: crate::Actor<ActorNames>,
        //M: 'static,
{
    fn clone(&self) -> Self {
        Endpoint {
            endpoint_fn: self.endpoint_fn,
        }
    }
}

impl<A, ActorNames> Addr<A, ActorNames>
 where ActorNames: MakeNamed,
{
    pub fn make_channel<Message>(&self) -> Endpoint<ActorNames, Message>
    where
        A : Handler<ActorNames, Message> + Actor<ActorNames>,
    {
        Endpoint {
            endpoint_fn: crate::scheduler::receive_for_endpoint::<ActorNames, A, Message>,
        }
    }
}
