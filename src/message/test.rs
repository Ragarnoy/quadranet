#[cfg(test)]
mod test {
    use crate::device::Uid;
    use crate::message::intent::Intent;
    use crate::message::Message;
    use core::convert::TryFrom;
    use core::num::NonZeroU8;

    #[test]
    fn test_serialize_message() {
        let sender_uid = Uid::try_from(NonZeroU8::new(0x01).unwrap()).unwrap();
        let receiver_uid = Uid::try_from(NonZeroU8::new(0x02).unwrap()).unwrap();
        let mut content: [u8; 64] = [0x0; 64];
        content[0..4].copy_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        let message = Message::data(sender_uid, receiver_uid, content);
        let bytes: [u8; 70] = message.into();
        assert_eq!(bytes[0], Intent::Data as u8);
        assert_eq!(bytes[1], sender_uid.get());
        assert_eq!(bytes[2], receiver_uid.get());
        assert_eq!(bytes[3], 0);
        assert_eq!(bytes[4], 4);
        assert_eq!(bytes[5], 0);
        assert_eq!(bytes[6..10], [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_deserialize_message() {
        let sender_uid = Uid::try_from(NonZeroU8::new(0x01).unwrap()).unwrap();
        let receiver_uid = Uid::try_from(NonZeroU8::new(0x02).unwrap()).unwrap();
        let mut content: [u8; 64] = [0x0; 64];
        content[0..4].copy_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        let message = Message::data(sender_uid, receiver_uid, content);
        let bytes: [u8; 70] = message.into();
        let deserialized_message = Message::try_from(&bytes[..]).unwrap();
        assert_eq!(deserialized_message.intent, Intent::Data);
        assert_eq!(deserialized_message.sender_uid, sender_uid);
        assert_eq!(deserialized_message.receiver_uid, Some(receiver_uid));
        assert_eq!(deserialized_message.ttl, 0);
        assert_eq!(deserialized_message.content, content);
    }

    #[test]
    fn test_message_ping() {
        let sender_uid = Uid::try_from(NonZeroU8::new(0x01).unwrap()).unwrap();
        let message = Message::ping(sender_uid);
        assert_eq!(message.intent, Intent::Ping, "Ping intent");
        assert_eq!(message.sender_uid, sender_uid);
        assert_eq!(message.receiver_uid, None);
        assert_eq!(message.length, 0);
        assert_eq!(message.ttl, 0);
        assert_eq!(message.content, [0u8; 64]);
    }

    #[test]
    fn test_message_data() {
        let sender_uid = Uid::try_from(NonZeroU8::new(0x01).unwrap()).unwrap();
        let receiver_uid = Uid::try_from(NonZeroU8::new(0x02).unwrap()).unwrap();
        let mut content: [u8; 64] = [0x0; 64];
        content[0..4].copy_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        let message = Message::data(sender_uid, receiver_uid, content);
        assert_eq!(message.intent, Intent::Data);
        assert_eq!(message.sender_uid, sender_uid);
        assert_eq!(message.receiver_uid, Some(receiver_uid));
        assert_eq!(message.length, 4);
        assert_eq!(message.ttl, 0);
        assert_eq!(message.content, content);
    }
}
