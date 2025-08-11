use crate::MakeNamed;
use std::ops::{IndexMut, Index};

pub struct EnumMap<T, E>
    where
        E: MakeNamed,
        T: Send,
{
    contents: E::ArrayType<T>,
}

impl<T, E> EnumMap<T, E>
    where
        E: MakeNamed,
        T: Send,
{
    pub fn new() -> EnumMap<T, E>
    where
        T: Default,
    {
        EnumMap {
            contents: E::array_from_fn(|_| T::default())
        }
    }

    pub fn from_fn<F>(f: F) -> EnumMap<T, E>
    where
        F: FnMut(E) -> T,
    {
        EnumMap {
            contents: E::array_from_fn(f)
        }
    }

    /// Returns the iter of this [`EnumMap<T, E>`].
    pub fn iter(&self) -> EnumMapIterator<'_, T, E> {
        EnumMapIterator {
            pos: 0,
            map: self,
        }
    }
}

pub struct EnumMapIterator<'a, T, E>
    where
        E: MakeNamed,
        T: Send,
{
    pos: usize,
    map: &'a EnumMap<T, E>,
}

impl<'a, T, E> Iterator for EnumMapIterator<'a, T, E>
    where
        E: MakeNamed,
        T: Send,
{
    type Item = (E, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        let i: usize = self.pos;
        if i < E::COUNT {
            self.pos = i + 1;
            let id = i.into();
            Some((id, E::index_array(&self.map.contents, id)))
        } else {
            None
        }
    }
}

impl<T, E> Index<E> for EnumMap<T, E>
    where
        E: MakeNamed,
        T: Send,
{
    type Output = T;
    fn index(&self, id: E) -> &T {
        E::index_array(&self.contents, id)
    }
}

impl<T, E> IndexMut<E> for EnumMap<T, E>
    where
        E: MakeNamed,
        T: Send
{
    fn index_mut(&mut self, id: E) -> &mut T {
        E::index_array_mut(&mut self.contents, id)
    }
}
