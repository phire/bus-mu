use strum::{EnumCount, IntoEnumIterator};

pub trait IdProvider<T> {
    fn id() -> T where Self: Sized;
}

pub struct ObjectMap<T, E> where
    E: EnumCount,
    [(); E::COUNT]: ,
        T: IdProvider<E> + ?Sized,
        usize: From<E> {
    contents: [Box<T>; E::COUNT]
}

impl<T, E> ObjectMap<T, E> where
    E: EnumCount + IntoEnumIterator,
    [(); E::COUNT]: ,
    T: IdProvider<E> + ?Sized,
    usize : From<E>
{
    pub fn new() -> ObjectMap<T, E>
    where
        Box<T>: From<E>,
        usize: From<E>, {
            let mut vec: Vec<Box<T>> = Vec::new();
            for e in E::iter() {
                vec.push(e.into());
            }
            assert!(vec.len() == E::COUNT);
        ObjectMap {
            // FIXME: If the caller doesn't correctly implement From<Box<T>> for E, then contents
            //        will be invalid and errors will spread to `get<U>`
            contents: unsafe {
                vec.try_into().unwrap_unchecked()
            }
        }
    }

    pub fn get<U>(&mut self) -> &mut U
        where U: IdProvider<E> + Sized
    {
        let index: usize = U::id().into();
        unsafe {
            // Safety: This is safe, but only if contents[U::id()] is actually a U.
            let obj = &mut self.contents[index];
            let u_obj: &mut Box<U> = std::mem::transmute(obj);
            return u_obj.as_mut();
        }
    }
    pub fn get_id(&mut self, id: E) -> &mut T
    {
        let index: usize = id.into();
        let obj = &mut self.contents[index];
        return obj.as_mut();
    }
}

