use crate::message::content::Content;
use crate::message::Message;

#[derive(Debug)]
pub enum StackError {
    StackFull,
    StackEmpty,
    NoLock,
}

pub trait MessageStack {
    fn push<C: Content>(&mut self, message: Message<C>) -> Result<(), StackError>
    where
        [(); C::SIZE]:;
    fn pop<C: Content>(&mut self) -> Result<Message<C>, StackError>
    where [(); C::SIZE]:;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}
