

use std::collections::BinaryHeap;

use crate::Time;

pub struct TimeQueue<T> {
    queue: BinaryHeap<TimeQueueEntry<T>>,
}

impl<T> TimeQueue<T> {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
        }
    }
    pub fn peek(&self) -> Option<(Time, &T)> {
        self.queue.peek()
            .map(|TimeQueueEntry{time, value}| (time.clone(), value))
    }
    pub fn pop(&mut self) -> Option<(Time, T)> {
       self.queue.pop()
            .map(|TimeQueueEntry{time, value}| (time.clone(), value))
    }
    pub fn push(&mut self, time: Time, value: T) {
        self.queue.push(TimeQueueEntry { time, value });
    }
}

struct TimeQueueEntry<T> {
    time: Time,
    value: T,
}

impl<T> PartialEq for TimeQueueEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}
impl<T> Eq for TimeQueueEntry<T> {}

impl<T> PartialOrd for TimeQueueEntry<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for TimeQueueEntry<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.cmp(&other.time).reverse()
    }
}
