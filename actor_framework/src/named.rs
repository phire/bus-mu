// TODO: Think of better name for named

use std::marker::PhantomData;

use crate::{ActorBox, Actor, AsBase};

pub trait Named<E> {
    fn name() -> E;
    fn dyn_name(&self) -> E;
    fn from_storage<'a, 'b>(storage: &'a mut E::StorageType) -> &'b mut ActorBox<E, Self>
    where
        'a: 'b,
        E: MakeNamed,
        Self: Actor<E> + Sized;
}

pub trait MakeNamed : From<usize> + Into<usize> + PartialEq + Copy + 'static + std::fmt::Debug + Send

{
    const COUNT: usize;
    const TERMINAL: Self; // Scheduler uses this as an actor that never get scheduled
    type Base<A> where A: Actor<Self>, Self: Sized;
    type StorageType: AsBase<Self> + Send;
    type ArrayType<T> : Send where T: Send;
    type Config : Send;

    fn storage_with(config: &Self::Config) -> Result<Self::StorageType, anyhow::Error>;

    fn iter() -> NamedIterator<Self> where Self: Sized, Self: From<usize> {
        NamedIterator {
            pos: 0,
            e_type: PhantomData::<*const Self>,
        }
    }

    fn index_array<T>(array: &Self::ArrayType<T>, id: Self) -> &T where T: Send;
    fn index_array_mut<T>(array: &mut Self::ArrayType<T>, id: Self) -> &mut T where T: Send;
    fn array_from_fn<T>(f: impl FnMut(Self) -> T) -> Self::ArrayType<T> where T: Send;

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

