use std::marker::PhantomData;


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

pub trait MakeNamed : From<usize> + Into<usize> + PartialEq + Copy + 'static + std::fmt::Debug {
    const COUNT: usize;
    fn iter() -> NamedIterator<Self> where Self: Sized, Self: From<usize> {
        NamedIterator {
            pos: 0,
            e_type: PhantomData::<*const Self>,
        }
    }

    fn make(id: Self) -> Box<dyn Named<Self>>;
}

pub struct ObjectMap<E>
    where
        E: MakeNamed,
        [(); E::COUNT]: ,
{
    contents: [Box<dyn Named<E>>; E::COUNT]
}

impl<E> ObjectMap<E> where
    E: MakeNamed,
    [(); E::COUNT]: ,
{
    pub fn new() -> ObjectMap<E>
    where
        usize: From<E>, {
            let mut vec: Vec<Box<dyn Named<E>>> = Vec::new();
            for e in E::iter() {
                let obj = E::make(e);
                // Safety: Make sure that the returned object still reports the same name
                assert!(obj.dyn_name() == e);

                vec.push(obj);
            }
            // Safety: Make sure we got the correct number of objects
            assert!(vec.len() == E::COUNT);
        ObjectMap {
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
    pub fn get_id(&mut self, id: E) -> &mut dyn Named<E>
    {
        let index: usize = id.into();
        let obj = &mut self.contents[index];
        return obj.as_mut();
    }
}
