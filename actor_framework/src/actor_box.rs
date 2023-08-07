
use crate::{MakeNamed, MessagePacketProxy, Actor, Outbox, ActorCreate};

#[repr(C)]
pub struct ActorBoxBase<ActorName>
where
    ActorName: MakeNamed,
{
    pub outbox: MessagePacketProxy<ActorName>,
}

#[repr(C)]
pub struct ActorBox<ActorNames, A>
where
    A: Actor<ActorNames>,
    A::OutboxType: Outbox<ActorNames>,
    ActorNames: MakeNamed,
{
    pub outbox: A::OutboxType,
    pub actor: A,
}

impl<ActorNames, A> Default for ActorBox<ActorNames, A>
where
    ActorNames: MakeNamed,
    A: Actor<ActorNames> + ActorCreate<ActorNames>,
    A::OutboxType: Outbox<ActorNames> + Default
{
    fn default() -> Self {
        let mut outbox = Default::default();
        let actor = <A as ActorCreate<ActorNames>>::new(&mut outbox, 0.into());
        ActorBox {
            outbox,
            actor
        }
    }
}

pub trait AsBase<ActorNames>
where
    ActorNames: MakeNamed,
{
    fn as_base<'a>(&'a self, id: ActorNames) -> &'a ActorBoxBase<ActorNames>;
}
