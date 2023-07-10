pub trait IdProvider<T> {
    fn id() -> T where Self: Sized;
}

pub fn new<T, E>(objs: [Box<T>; std::mem::variant_count::<E>()]) -> ObjectMap<T, E>
where [(); std::mem::variant_count::<E>()]: ,
    T: IdProvider<E> + ?Sized,
    usize: From<E>, {
        // FIXME: this isn't safe. No way to be sure objs was correct
    ObjectMap {
        contents: objs
    }
}

pub struct ObjectMap<T, E> where
    [(); std::mem::variant_count::<E>()]: ,
        T: IdProvider<E> + ?Sized,
        usize: From<E> {
    contents: [Box<T>; std::mem::variant_count::<E>()]
}

impl<T, E> ObjectMap<T, E> where
    [(); std::mem::variant_count::<E>()]: ,
    T: IdProvider<E> + ?Sized,
    usize : From<E>
{
    pub fn get<U>(&mut self) -> &mut U
        where U: IdProvider<E> + Sized
    {
        let index: usize = U::id().into();
        unsafe {
            // Safety:
            let obj = &mut self.contents[index];
            let u_obj: &mut Box<U> = std::mem::transmute(obj);
            return u_obj.as_mut();
        }
    }
    pub fn get_id(&mut self, id: E) -> &mut T
    {
        let index: usize = id.into();
        let obj = &mut self.contents[index];
        return  obj.as_mut();
    }
}

