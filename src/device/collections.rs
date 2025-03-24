use defmt::Format;

use crate::message::Message;

#[derive(Debug, Format)]
pub enum CollectionError {
    Empty,
    Full,
    NoLock,
}

pub trait MessageStack {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn pop(&mut self) -> Result<Message, CollectionError>;
    fn push(&mut self, message: Message) -> Result<(), CollectionError>;
}

pub trait MessageQueue {
    fn enqueue(&mut self, message: Message) -> Result<(), CollectionError>;
    fn dequeue(&mut self) -> Result<Message, CollectionError>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}
