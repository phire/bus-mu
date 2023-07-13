use std::{marker::PhantomData, ops::{IndexMut, Index}};

pub trait Named<E> {
    fn name() -> E where Self: Sized;
    fn dyn_name(&self) -> E;
}

pub struct NamedIterator<E> {
    pos: usize,
    e_type: PhantomData<*const E>,
}

impl<E> Iterator for NamedIterator<E> where E: MakeNamed {
    type Item = E;

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

pub trait MakeNamed : From<usize> + Into<usize> + PartialEq + Copy + 'static + std::fmt::Debug
{
    const COUNT: usize;
    type Base : Named<Self> + ?Sized;
    //type Storage = Box<Self::Super>;
    fn iter() -> NamedIterator<Self> where Self: Sized, Self: From<usize> {
        NamedIterator {
            pos: 0,
            e_type: PhantomData::<*const Self>,
        }
    }

    fn make(id: Self) -> Box<Self::Base>;
}

pub struct ObjectStore<E>
    where
        E: MakeNamed,
        [(); E::COUNT]: ,
{
    contents: [Box<E::Base>; E::COUNT]
}

impl<E> ObjectStore<E> where
    E: MakeNamed,
    E::Base: Named<E>,
    [(); E::COUNT]: ,
{
    pub fn new() -> ObjectStore<E>
    where
        usize: From<E>, {
            let mut vec: Vec<Box<E::Base>> = Vec::new();
            for e in E::iter() {
                let obj = E::make(e);
                // Safety: Make sure that the returned object still reports the same name
                assert!(obj.dyn_name() == e);

                vec.push(obj);
            }
            // Safety: Make sure we got the correct number of objects
            assert!(vec.len() == E::COUNT);
        ObjectStore {
            // Safety: Will be safe because of above assertions
            contents: unsafe {
                vec.try_into().unwrap_unchecked()
            }
        }
    }

    pub fn get<U>(&mut self) -> &mut U
        where U: Named<E> + Sized
    {
        let index: usize = U::name().into();
        unsafe {
            // Safety: This is safe, but only if contents[U::id()] is actually a U.
            let obj = &mut self.contents[index];
            let u_obj: &mut Box<U> = std::mem::transmute(obj);
            return u_obj.as_mut();
        }
    }
    pub fn get_id(&mut self, id: E) -> &mut E::Base
    {
        let index: usize = id.into();
        let obj = &mut self.contents[index];
        return obj;
    }
}

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
        T: Default + Copy,
        usize: From<E>, {
        EnumMap {
            contents: [T::default(); E::COUNT]
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
