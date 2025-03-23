use core::convert::TryFrom;

use postcard::{from_bytes, to_allocvec};

use crate::device::Uid;
use crate::message::payload::data::DataType;
use crate::message::payload::Payload;
use crate::message::Message;

#[test]
fn test_message() {
    let source_id = Uid::try_from(0x01).unwrap();
    let destination_id = Uid::try_from(0x02).unwrap();
    let payload = Payload::Data(DataType::new_text("Hello World!"));
    let ttl = 10;

    let message = Message::new(source_id, Some(destination_id), payload.clone(), ttl, false);

    assert_eq!(message.source_id(), source_id);
    assert_eq!(message.destination_id(), Some(destination_id));
    assert_eq!(message.payload(), &payload);
    assert_eq!(message.ttl(), ttl);
    assert!(message.is_expired());
    assert!(message.is_for_me(destination_id));
    assert!(message.is_for_me(source_id));
}

#[test]
fn test_message_decrement_ttl() {
    let source_id = Uid::try_from(0x01).unwrap();
    let destination_id = Uid::try_from(0x02).unwrap();
    let payload = Payload::Data(DataType::new_text("Hello World!"));
    let ttl = 10;

    let mut message = Message::new(source_id, Some(destination_id), payload, ttl, false);

    assert_eq!(message.ttl(), ttl);
    message.decrement_ttl();
    assert_eq!(message.ttl(), ttl - 1);
}

#[test]
fn test_message_is_expired() {
    let source_id = Uid::try_from(0x01).unwrap();
    let destination_id = Uid::try_from(0x02).unwrap();
    let payload = Payload::Data(DataType::new_text("Hello World!"));
    let ttl = 10;

    let mut message = Message::new(source_id, Some(destination_id), payload, ttl, false);

    assert!(message.is_expired());
    for _ in 0..ttl {
        message.decrement_ttl();
    }
    assert!(message.is_expired());
}

#[test]
fn test_message_serialization_deserialization() {
    let source_id = Uid::try_from(0x01).unwrap();
    let destination_id = Uid::try_from(0x02).unwrap();
    let payload = Payload::Data(DataType::new_text("Hello World!"));
    let ttl = 10;

    let message = Message::new(source_id, Some(destination_id), payload.clone(), ttl, false);
    let serialized = to_allocvec(&message).unwrap();
    let deserialized: Message = from_bytes(&serialized).unwrap();

    assert_eq!(deserialized.source_id(), source_id);
    assert_eq!(deserialized.destination_id(), Some(destination_id));
    assert_eq!(deserialized.payload(), &payload);
    assert_eq!(deserialized.ttl(), ttl);
}

#[test]
fn test_invalid_serialization_data() {
    let invalid_data = [0u8; 70]; // Assuming this is an invalid data for your message format
    let result: Result<Message, _> = from_bytes(&invalid_data);

    assert!(result.is_err());
}

#[test]
fn test_broadcast_message_creation() {
    let source_id = Uid::try_from(0x01).unwrap();
    let payload = Payload::Data(DataType::new_text("Hello World!"));
    let ttl = 10;

    let message = Message::new(source_id, None, payload, ttl, false);

    assert_eq!(message.destination_id(), None);
    assert!(message.is_for_me(Uid::try_from(0x02).unwrap())); // Assuming 'is_for_me' checks if the message is a broadcast
}

#[test]
fn test_message_for_me() {
    let source_id = Uid::try_from(0x01).unwrap();
    let destination_id = Uid::try_from(0x02).unwrap();
    let payload = Payload::Data(DataType::new_text("Hello World!"));
    let ttl = 10;

    let message = Message::new(source_id, Some(destination_id), payload, ttl, false);

    assert!(message.is_for_me(destination_id));
    assert!(message.is_for_me(source_id));
}
