use defmt::Format;

use crate::message::Message;

#[derive(Debug, Format)]
pub enum CollectionError {
    Full,
    Empty,
    NoLock,
}

pub trait MessageStack {
    fn push(&mut self, message: Message) -> Result<(), CollectionError>;
    fn pop(&mut self) -> Result<Message, CollectionError>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

pub trait MessageQueue {
    fn enqueue(&mut self, message: Message) -> Result<(), CollectionError>;
    fn dequeue(&mut self) -> Result<Message, CollectionError>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}
