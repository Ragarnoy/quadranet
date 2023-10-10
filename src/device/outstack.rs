use heapless::Vec;
use crate::message::Message;


pub struct OutStack {
    stack: Vec<Message, 32>,
}

impl OutStack {
    pub const fn new() -> Self {
        Self {
            stack: Vec::new(),
        }
    }

    pub fn push(&mut self, message: Message) {
        self.stack.push(message).unwrap();
    }

    pub fn pop(&mut self) -> Option<Message> {
        self.stack.pop()
    }
}
