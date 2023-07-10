use std::marker::PhantomData;
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

    pub fn make_handle<U>(&self) -> Handle<T, U, E>
        where U: IdProvider<E>,
              T: Sized,
    {
        Handle {
            id: U::id(),
            // base: PhantomData::<T>,
            // cast: PhantomData::<U>,
            caster: PhantomData::<dyn Caster<T, U>>,
        }
    }
}

pub trait Caster<From, To>  {
    unsafe fn do_cast<'a>(&self, val: &'a mut From) -> &'a mut To;
}

impl<From, To> Caster<From, To> for PhantomData::<dyn Caster<From, To>>
{
    unsafe fn do_cast<'a>(&self, val: &'a mut From) -> &'a mut To {
        std::mem::transmute(val)
    }
}

#[derive(Copy, Clone)]
pub struct Handle<T, U, E>
    where U : ?Sized,
        T : ?Sized,
        usize: From<E>,{
    // SAFETY: It's essential that this id is correct
    id: E,
    caster: PhantomData<dyn Caster<T, U>>,
    //base: PhantomData<T>,
    //cast: PhantomData<U>,
}

impl<T, U, E> Handle<T, U, E>
    where [(); std::mem::variant_count::<E>()]: ,
    U: ?Sized,
    T: ?Sized,
    usize: From<E>,
    T: IdProvider<E>,
    E: Copy
{

    pub fn new() -> Handle<T, U, E>
        where U: IdProvider<E> + Sized,
    {
        Handle {
            id: U::id(),
            //base: PhantomData::<T>,
            //cast: PhantomData::<U>,
            caster: PhantomData::<dyn Caster<T, U>>,
        }
    }
    pub fn get<'a>(&self, map: &'a mut ObjectMap<T, E>) -> &'a mut U
    {
        let index: usize = self.id.into();
        unsafe {
            // Safety:
            let obj = &mut map.contents[index];
            //let u_obj: &mut Box<U> = std::mem::transmute(obj);
            return self.caster.do_cast(obj.as_mut());

        }
    }

    pub fn cast<V>(&self) -> Handle<T, V, E> where
        V: ?Sized,
        PhantomData<dyn Caster<U, V>>:
        {

            Handle {
                id: self.id,
                // base: PhantomData::<U>,
                // cast: PhantomData::<V>,
                caster: PhantomData::<dyn Caster<T, V>>,
            }
    }
}
