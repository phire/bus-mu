// TODO: Think of better name for named

use std::marker::PhantomData;

use crate::{ActorBox, Actor};

pub trait Named<E> {
    fn name() -> E where Self: Sized;
    fn dyn_name(&self) -> E;
    fn from_storage<'a, 'b>(storage: &'a mut E::StorageType) -> &'b mut ActorBox<E, Self>
    where
        'a: 'b,
        E: MakeNamed,
        Self: Actor<E> + Sized;
}

pub trait MakeNamed : From<usize> + Into<usize> + PartialEq + Copy + 'static + std::fmt::Debug
where
    Self::StorageType: Default,
{
    const COUNT: usize;
    type Base<A> where A: Actor<Self>, Self: Sized;
    type ExitReason = Box<dyn std::error::Error>;
    type StorageType;

    fn iter() -> NamedIterator<Self> where Self: Sized, Self: From<usize> {
        NamedIterator {
            pos: 0,
            e_type: PhantomData::<*const Self>,
        }
    }

    //fn make(id: Self) -> Pin<Box<Self::Base>>;

    fn size_of(id: Self) -> usize;
}

pub struct NamedIterator<E> {
    pos: usize,
    e_type: PhantomData<*const E>,
}

impl<E> Iterator for NamedIterator<E> where E: MakeNamed {
    type Item = E;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let i: usize = self.pos;
        if i < E::COUNT {
            self.pos = i + 1;
            Some(i.into())
        } else {
            None
        }
    }
}

