use crate::message::Message;

#[derive(Debug)]
pub enum StackError {
    StackFull,
    StackEmpty,
    NoLock,
}

pub trait MessageStack {
    fn push(&mut self, message: Message) -> Result<(), StackError>;
    fn pop(&mut self) -> Result<Message, StackError>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}
