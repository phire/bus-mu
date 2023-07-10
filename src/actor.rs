

use std::{marker::PhantomData};


use crate::object_map::{ObjectMap, IdProvider};

struct MessageA {}
struct MessageB {}
struct MessageC {}

#[derive(Copy, Clone)]
enum ActorId {
    TestA,
    TestB,
}

impl From<ActorId> for usize {
    fn from(id: ActorId) -> usize {
        id as usize
    }
}

//trait Message {}

trait MessagePacketInner {
    fn execute(self: Box<Self>, map: &mut ObjectMap<dyn IdProvider<ActorId>, ActorId>) -> MessagePacket;
}

trait Handler<M> {
    fn recv(&mut self, message: M) -> MessagePacket;
}


pub struct MessagePacket {
    inner: Option<Box<dyn MessagePacketInner>>,
    time: Time,
}

impl MessagePacket {
    pub fn no_message() -> MessagePacket {
        MessagePacket {
            inner: None,
            time: Time {
                cycles: 0,
            },
        }
    }
}

pub struct MessagePacketImpl<A, M> {
    message: M,
    type_id: PhantomData<*const A>,
}

impl<A, M> MessagePacketInner for MessagePacketImpl<A, M>
     where A: IdProvider<ActorId> + Handler<M>
{
    fn execute(self: Box<Self>, map: &mut ObjectMap<dyn IdProvider<ActorId>, ActorId>) -> MessagePacket {
        map.get::<A>().recv(self.message)
    }
}

pub struct MessagePacketChannel<M> {
    message: M,
    channel_fn: fn (map: &mut ObjectMap<dyn IdProvider<ActorId>, ActorId>, message: M) -> MessagePacket,
}

impl<M> MessagePacketInner for MessagePacketChannel<M>
{
    fn execute(self: Box<Self>, map: &mut ObjectMap<dyn IdProvider<ActorId>, ActorId>) -> MessagePacket {
        (self.channel_fn)(map, self.message)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Time {
    // TODO: allow lazy times
    cycles: u64
}

fn channel_fn<A, M>(map: &mut ObjectMap<dyn IdProvider<ActorId>, ActorId>, message: M) -> MessagePacket
where A: IdProvider<ActorId> + Handler<M>,
      M: 'static,
{
    map.get::<A>().recv(message)
}

struct Addr<Actor> {
    actor_type: PhantomData<*const Actor>,
}

impl<A> Addr<A> {
    pub fn send<M>(&self, message: M, time: Time) -> MessagePacket
    where A: Handler<M>,
          M: 'static,
          A: IdProvider<ActorId> + 'static,
    {
        // TODO: Don't box messages
        MessagePacket {
            inner: Some(Box::new(MessagePacketImpl::<A, M>
            {
                message: message,
                type_id: PhantomData::<*const A>,
            })),
            time,
        }
    }

    pub fn make_channel<M>(&self) -> Channel<M>
    where A : Handler<M>,
          A: IdProvider<ActorId> + 'static,
          M: 'static,
    {
        Channel {
            channel_fn: channel_fn::<A, M>,
        }
    }
}

pub struct Channel<M> {
    channel_fn: fn (map: &mut ObjectMap<dyn IdProvider<ActorId>, ActorId>, message: M) -> MessagePacket,
}

impl<M> Channel<M>
    where M: 'static
{
    pub fn send(&self, message: M, time: Time) -> MessagePacket {
        MessagePacket {
            inner: Some(Box::new(MessagePacketChannel::<M>
            {
                message: message,
                channel_fn: self.channel_fn,
            })),
            time,
        }
    }
}

pub trait Actor {
    fn run(&mut self, max_cycles: u64) -> MessagePacket;
}



#[derive(Default)]
struct TestA {
    self_addr: Option<Addr<TestA>>,
}

impl Actor for TestA {
    fn run(&mut self, max_cycles: u64) -> MessagePacket {
        MessagePacket::no_message()
    }
}

impl IdProvider<ActorId> for TestA {
    fn id() -> ActorId {
        ActorId::TestA
    }
}

impl Handler<MessageA> for TestA {
    fn recv(&mut self, message: MessageA) -> MessagePacket {
        println!("TestA recv MessageA");
        drop(message);
        return MessagePacket::no_message();
    }
}

#[derive(Default)]
struct TestB {

}

impl Actor for TestB {
    fn run(&mut self, max_cycles: u64) -> MessagePacket {
        MessagePacket::no_message()
    }
}

impl IdProvider<ActorId> for TestB {
    //const ID: ActorId = ActorId::TestB;
    fn id() -> ActorId {
        ActorId::TestB
    }
}

mod object_map {

}

trait ActorWithId : Actor + IdProvider<ActorId> {}

pub struct Scheduler {
    actors: ObjectMap<dyn IdProvider<ActorId>, ActorId>,
    commited_time: Time,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            actors: crate::object_map::new(
                [
                    Box::new(TestA::default()),
                    Box::new(TestB::default()),
                ]
            ),
            commited_time: Time { cycles: 0 }
        }
    }

    pub fn run(&mut self) {
        let mut message = MessagePacket::no_message();

        let addr_a = Addr::<TestA> { actor_type: PhantomData::<*const TestA> };
        let _addr_a = Addr::<TestB> { actor_type: PhantomData::<*const TestB> };
        let _chan_a = addr_a.make_channel::<MessageA>();
        //let chanA = addrA.make_channel::<MessageB>();


        message = addr_a.send(MessageA{}, Time { cycles: 0 });

        loop {
            match message {
                MessagePacket { inner: None, time: _ } => {
                    // Find the actor with the smallest window and advance it's time
                }
                MessagePacket { inner: Some(m), time } => {
                    match time {
                        time if time == self.commited_time => {
                            // We have a message for the current time, deliver it
                            message = m.execute(&mut self.actors);
                        }
                        time if time > self.commited_time => {
                            // We have a message for the future, we need to advance all actors to that time
                            todo!();
                        }
                        _ => {
                            panic!("Message sent to the past")
                        }
                    }
                }
            }
        }
    }
}
