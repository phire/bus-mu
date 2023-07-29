
use crate::{MakeNamed, Named};

pub struct ObjectStore<E>
    where
        E: MakeNamed,
        [(); E::COUNT]: ,
{
    contents: [Box<E::Base>; E::COUNT]
}

impl<E> ObjectStore<E> where
    E: MakeNamed,
    [(); E::COUNT]: ,
{
    pub fn new() -> ObjectStore<E> where usize: From<E>
    {
        ObjectStore { contents: std::array::from_fn(|i| {
            let name = E::from(i);
            let obj = E::make(name);
            // Safety: Make sure that the returned object still reports the same name
            assert!(obj.dyn_name() == name);
            obj
        }) }
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
