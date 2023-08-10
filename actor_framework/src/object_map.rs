use crate::{MakeNamed, Named, Actor, ActorBox, Outbox, ActorBoxBase, actor_box::AsBase};

pub struct ObjectStore<E>
    where
        E: MakeNamed,
{
    storage: E::StorageType,
}

impl<E> ObjectStore<E> where
    E: MakeNamed,
{
    pub fn new() -> ObjectStore<E>
    where
        E::StorageType: Default,
    {
        Self {
            storage: Default::default()
        }
    }

    #[inline(always)]
    pub fn get<'a, 'b, U>(&'a mut self) -> &'b mut ActorBox<E, U>
    where
        U: Named<E> + Sized + Actor<E> + 'b,
        <U as Actor<E>>::OutboxType: Outbox<E>,
        'a: 'b,
    {
        U::from_storage(&mut self.storage)
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub fn get_view<'a, 'b, U>(&'a mut self) -> ObjectStoreView<'b, E, ActorBox<E, U>>
    where
        U: Named<E> + Sized + Actor<E> + 'b,
        <U as Actor<E>>::OutboxType: Outbox<E>,
        'a: 'b,
    {
        let self_ptr = self as *mut Self;
        let data = unsafe { self_ptr.as_mut().unwrap_unchecked().get() };
        ObjectStoreView { data , storage: self_ptr }
    }

    #[inline(always)]
    pub fn get_base<'a>(&'a self, id: E) -> &'a ActorBoxBase<E>
    where
        E::StorageType: AsBase<E>,
    {
        self.storage.as_base(id)
    }
}

pub struct ObjectStoreView<'a, E, U>
    where
        E: MakeNamed,
{
    data: &'a mut U,
    storage: * mut ObjectStore<E>,
}

#[allow(dead_code)]
impl<'a, E, U> ObjectStoreView<'a, E, U>
    where
        E: MakeNamed,
{
    #[inline(always)]
    pub fn map<F, R>(self, f: F) -> ObjectStoreView<'a, E, R>
    where
        F: FnOnce(&'a mut U) -> &'a mut R,
    {
        let result = f(self.data);

        ObjectStoreView { data: result, storage: self.storage }
    }

    #[inline(always)]
    pub fn run<'b, 'r, F, R>(&'b mut self, f: F) -> R
    where
        F: FnOnce(&'b mut U) -> R,
    {
        let result = f(self.data);
        result
    }

    #[inline(always)]
    pub fn close<'b>(self) -> &'b mut ObjectStore<E> {
        unsafe {
            self.storage.as_mut().unwrap_unchecked()
        }
    }

    // #[inline(always)]
    // pub fn get_obj<'b, 'c, V>(&'b mut self) -> &'b mut ActorBox<E, V>
    // where
    //     V: Named<E> + Sized + Actor<E> + 'b,
    //     <V as Actor<E>>::OutboxType: Outbox<E>,
    //     'a: 'b,
    // {
    //     unsafe {
    //         self.storage.as_mut().unwrap_unchecked().get()
    //     }
    // }
}
