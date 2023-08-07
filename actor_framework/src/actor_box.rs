
use std::{ops::DispatchFromDyn, default};

use crate::{MakeNamed, MessagePacketProxy, Actor, Outbox, Handler};

#[repr(C)]
pub struct ActorBoxBase<ActorName>
where
    ActorName: MakeNamed,
{
    pub outbox: MessagePacketProxy<ActorName>,
}

#[repr(C)]
//#[derive(Default)]
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
    A: Actor<ActorNames> + Default,
    A::OutboxType: Outbox<ActorNames> + Default
{
    fn default() -> Self {
        ActorBox {
            outbox: Default::default(),
            actor: A::default(),
        }
    }
}

// impl<ActorNames, A> std::ops::Deref for ActorBox<ActorNames, A>
// where
//     A: Actor<ActorNames>,
//     A::OutboxType: Outbox<ActorNames>,
//     ActorNames: MakeNamed,
//     [(); ActorNames::COUNT]:
// {
//     type Target = A;

//     fn deref(&self) -> &Self::Target {
//         &self.actor
//     }
// }

// impl<ActorNames, A> std::ops::Deref for ActorBox<ActorNames, A>
// where
//     A: Actor<ActorNames>,
//     A::OutboxType: Outbox<ActorNames>,
//     ActorNames: MakeNamed,
//     [(); ActorNames::COUNT]:
// {
//     type Target = dyn Actor<ActorNames, OutboxType=A::OutboxType>;

//     fn deref(&self) -> &Self::Target {
//         &self.actor
//     }
// }

// impl<ActorNames, A> std::ops::DerefMut for ActorBox<ActorNames, A>
// where
//     A: Actor<ActorNames>,
//     A::OutboxType: Outbox<ActorNames>,
//     ActorNames: MakeNamed,
//     [(); ActorNames::COUNT]:
// {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.actor
//     }
// }

pub trait AsBase<ActorNames>
where
    ActorNames: MakeNamed,
{
    fn as_base<'a>(&'a self, id: ActorNames) -> &'a ActorBoxBase<ActorNames>;
}
