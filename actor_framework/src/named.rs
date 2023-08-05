// TODO: Think of better name for named

use std::marker::PhantomData;

pub trait Named<E> {
    fn name() -> E where Self: Sized;
    fn dyn_name(&self) -> E;
}

pub trait MakeNamed : From<usize> + Into<usize> + PartialEq + Copy + 'static + std::fmt::Debug
{
    const COUNT: usize;
    type Base : Named<Self> + ?Sized;
    type ExitReason = Box<dyn std::error::Error>;
    fn iter() -> NamedIterator<Self> where Self: Sized, Self: From<usize> {
        NamedIterator {
            pos: 0,
            e_type: PhantomData::<*const Self>,
        }
    }

    fn make(id: Self) -> Box<Self::Base>;
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
