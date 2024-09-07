use core::fmt::{Display, Formatter};
use defmt::Format;
use serde::{Deserialize, Deserializer, Serialize};

use crate::message::payload::MAX_PAYLOAD_SIZE;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Format)]
pub enum DataType {
    Text(Text),
    Binary(Binary),
}

impl DataType {
    pub fn new_text(text: &str) -> Self {
        let bytes = text.as_bytes();
        let len = bytes.len().min(MAX_PAYLOAD_SIZE);
        let mut data = [0u8; MAX_PAYLOAD_SIZE];
        data[..len].copy_from_slice(&bytes[..len]);
        DataType::Text(Text { data, len })
    }

    pub fn new_binary(bytes: &[u8]) -> Self {
        let mut data = [0; MAX_PAYLOAD_SIZE];
        let len = bytes.len().min(MAX_PAYLOAD_SIZE);
        data[..len].copy_from_slice(&bytes[..len]);
        DataType::Binary(Binary(data))
    }
}

#[derive(Clone, Debug, PartialEq, Format)]
pub struct Text {
    data: [u8; MAX_PAYLOAD_SIZE],
    len: usize,
}

impl Display for Text {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match core::str::from_utf8(&self.data[..self.len]) {
            Ok(content) => write!(f, "{}", content),
            Err(_) => write!(f, "<invalid UTF-8>"),
        }
    }
}

impl Serialize for Text {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let text = core::str::from_utf8(&self.data[..self.len])
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(text)
    }
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TextVisitor;

        impl<'de> serde::de::Visitor<'de> for TextVisitor {
            type Value = Text;

            fn expecting(&self, formatter: &mut Formatter) -> core::fmt::Result {
                formatter.write_str("a byte array representing UTF-8 text")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // Validate UTF-8
                core::str::from_utf8(v).map_err(E::custom)?;

                let len = v.len().min(MAX_PAYLOAD_SIZE);
                let mut data = [0u8; MAX_PAYLOAD_SIZE];
                data[..len].copy_from_slice(&v[..len]);
                Ok(Text { data, len })
            }
        }

        deserializer.deserialize_bytes(TextVisitor)
    }
}

#[derive(Clone, Debug, PartialEq, Format)]
pub struct Binary([u8; MAX_PAYLOAD_SIZE]);

impl Serialize for Binary {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Binary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes = <&[u8]>::deserialize(deserializer)?;
        if bytes.len() > MAX_PAYLOAD_SIZE {
            return Err(serde::de::Error::custom(
                "Binary data exceeds maximum payload size",
            ));
        }
        let mut data = [0; MAX_PAYLOAD_SIZE];
        data[..bytes.len()].copy_from_slice(bytes);
        Ok(Binary(data))
    }
}

#[cfg(test)]
mod test {
    use postcard::{from_bytes, to_allocvec};
    use crate::device::Uid;
    use crate::message::Message;
    use crate::message::payload::{Payload, MAX_PAYLOAD_SIZE};
    use crate::message::payload::data::DataType;

    #[test]
    fn test_message_serialization_deserialization_thorough() {
        let max_size = "a".repeat(MAX_PAYLOAD_SIZE);
        let over_size = "a".repeat(MAX_PAYLOAD_SIZE + 1);
        let test_cases = [
            "",
            "Hello, World!",
            &max_size,
            &over_size,
            "Hello\0World", // with null byte
            "🦀Rust🦀",     // UTF-8 multi-byte characters
        ];

        for case in test_cases.iter() {
            let original_payload = Payload::Data(DataType::new_text(case));
            let original_message = Message::new(
                Uid::try_from(1).unwrap(),
                Some(Uid::try_from(2).unwrap()),
                original_payload.clone(),
                10,
                false,
            );

            let serialized = to_allocvec(&original_message).unwrap();
            let deserialized: Message = from_bytes(&serialized).unwrap();

            assert_eq!(original_message, deserialized);
            assert_eq!(original_payload, *deserialized.payload());

            if let Payload::Data(DataType::Text(text)) = deserialized.payload() {
                let deserialized_str = core::str::from_utf8(&text.data[..text.len]).unwrap();
                let expected = if case.len() > MAX_PAYLOAD_SIZE {
                    &case[..MAX_PAYLOAD_SIZE]
                } else {
                    case
                };
                assert_eq!(deserialized_str, expected);
            } else {
                panic!("Expected Data payload");
            }
        }
    }
}
