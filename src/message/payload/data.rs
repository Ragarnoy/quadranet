use crate::message::payload::MAX_PAYLOAD_SIZE;
use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Format)]
pub enum DataType {
    Text(Text),
    Binary(Binary),
}

impl DataType {
    pub fn new_text(text: &str) -> Self {
        let mut data = [0; MAX_PAYLOAD_SIZE];
        let len = text.len().min(MAX_PAYLOAD_SIZE);
        data[..len].copy_from_slice(&text.as_bytes()[..len]);
        DataType::Text(Text(data))
    }

    pub fn new_binary(bytes: &[u8]) -> Self {
        let mut data = [0; MAX_PAYLOAD_SIZE];
        let len = bytes.len().min(MAX_PAYLOAD_SIZE);
        data[..len].copy_from_slice(&bytes[..len]);
        DataType::Binary(Binary(data))
    }
}

#[derive(Clone, Debug, PartialEq, Format)]
pub struct Text([u8; MAX_PAYLOAD_SIZE]);

impl Serialize for Text {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let text = core::str::from_utf8(&self.0).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(text)
    }
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let text = <&str>::deserialize(deserializer)?;
        if text.len() > MAX_PAYLOAD_SIZE {
            return Err(serde::de::Error::custom(
                "Text data exceeds maximum payload size",
            ));
        }
        let mut bytes = [0; MAX_PAYLOAD_SIZE];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        Ok(Text(bytes))
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
