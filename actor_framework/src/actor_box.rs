
use crate::{MakeNamed, MessagePacketProxy, Actor, Outbox, ActorInit};

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
    pub obj: A,
}

impl<ActorNames, A> ActorBox<ActorNames, A>
where
    ActorNames: MakeNamed,
    A: Actor<ActorNames> + ActorInit<ActorNames>,
    A::OutboxType: Outbox<ActorNames> + Default
{
    pub fn with(config: &ActorNames::Config) -> Result<Self, anyhow::Error> {
        let mut outbox = Default::default();
        let time = 0.into();
        let actor = <A as ActorInit<ActorNames>>::init(config, &mut outbox, time)?;
        Ok(ActorBox {
            outbox,
            obj: actor
        })
    }
}

pub trait AsBase<ActorNames>
where
    ActorNames: MakeNamed,
{
    fn as_base<'a>(&'a self, id: ActorNames) -> &'a ActorBoxBase<ActorNames>;
}
