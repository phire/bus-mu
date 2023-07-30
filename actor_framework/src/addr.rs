use std::marker::PhantomData;

use crate::{MakeNamed, Named};

pub struct Addr<Actor, Name> {
    actor_type: PhantomData<*const Actor>,
    named_type: PhantomData<*const Name>,
}

impl<Actor, Name> Default for Addr<Actor, Name> {
    fn default() -> Self {
        Self {
            actor_type: PhantomData::<*const Actor>,
            named_type: PhantomData::<*const Name>,
        }
    }
}

trait MakeAddr<Name> where Self: Sized, Name: MakeNamed, [(); Name::COUNT]: {
    fn make_addr() -> Addr<Self, Name>;
}

impl<Name, A> MakeAddr<Name> for A where
    Name: MakeNamed,
    A: Named<Name>,
    [(); Name::COUNT]:
{
    fn make_addr() -> Addr<Self, Name> {
        Addr {
            actor_type: PhantomData::<*const Self>,
            named_type: PhantomData::<*const Name>,
        }
    }
}

