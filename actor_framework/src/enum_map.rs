use crate::MakeNamed;
use std::ops::{IndexMut, Index};

pub struct EnumMap<T, E>
    where
        E: MakeNamed,
        [(); E::COUNT]: ,
{
    contents: [T; E::COUNT]
}

impl<T, E> EnumMap<T, E>
    where E: MakeNamed,
          [(); E::COUNT]: ,
{
    pub fn new() -> EnumMap<T, E>
    where
        T: Default,
        usize: From<E>,
    {
        EnumMap {
            contents: std::array::from_fn(|_| T::default())
        }
    }

    /// Returns the iter of this [`EnumMap<T, E>`].
    pub fn iter(&self) -> EnumMapIterator<T, E> {
        EnumMapIterator {
            pos: 0,
            map: self,
        }
    }
}

pub struct EnumMapIterator<'a, T, E>
    where
        E: MakeNamed,
        [(); E::COUNT]: ,
{
    pos: usize,
    map: &'a EnumMap<T, E>,
}

impl<'a, T, E> Iterator for EnumMapIterator<'a, T, E>
    where E: MakeNamed,
          [(); E::COUNT]: ,
{
    type Item = (E, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        let i: usize = self.pos;
        if i < E::COUNT {
            self.pos = i + 1;
            Some((i.into(), &self.map.contents[i]))
        } else {
            None
        }
    }
}

impl<T, E> Index<E> for EnumMap<T, E>
    where E: MakeNamed,
          [(); E::COUNT]: ,
{
    type Output = T;
    fn index(&self, index: E) -> &T {
        &self.contents[index.into()]
    }
}

impl<T, E> IndexMut<E> for EnumMap<T, E>
    where E: MakeNamed,
          [(); E::COUNT]: ,
{
    fn index_mut(&mut self, index: E) -> &mut T {
        &mut self.contents[index.into()]
    }
}
