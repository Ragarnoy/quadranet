use core::convert::TryFrom;
use core::sync::atomic::{AtomicU32, Ordering};

use defmt::Format;
use serde::{Deserialize, Serialize};

use crate::device::config::device_config::DeviceConfig;
use crate::device::Uid;
use crate::message::error::MessageError;
use crate::message::payload::ack::AckType;
use crate::message::payload::command::CommandType;
use crate::message::payload::data::DataType;
use crate::message::payload::discovery::DiscoveryType;
use crate::message::payload::route::RouteType;
use payload::Payload;

pub mod error;
pub mod payload;

const MAX_TTL: u8 = 10;
const MAX_MESSAGE_SIZE: usize = 70;

// Use atomic for thread-safe message ID generation
static MESSAGE_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

fn generate_message_id() -> u32 {
    MESSAGE_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Format)]
pub struct Message {
    message_id: u32,
    /// Source ID is the UID of the node that sent the message
    source_id: Uid,
    /// Destination ID is the UID of the node the message is intended for
    destination_id: Option<Uid>,
    /// Time to live is the number of hops a message can take before it is considered expired
    ttl: u8,
    /// Req ack is a flag that indicates if the message requires an acknowledgement
    req_ack: bool,
    /// Payload is the data being sent
    payload: Payload,
}

impl Message {
    pub fn new(
        source_id: Uid,
        destination_id: Option<Uid>,
        payload: Payload,
        ttl: u8,
        require_ack: bool,
    ) -> Self {
        Self {
            message_id: generate_message_id(),
            source_id,
            destination_id,
            payload,
            req_ack: require_ack,
            ttl: ttl.min(MAX_TTL),
        }
    }

    pub fn new_data(
        source_id: Uid,
        destination_id: Option<Uid>,
        payload: DataType,
        ttl: u8,
        require_ack: bool,
    ) -> Self {
        Self::new(
            source_id,
            destination_id,
            Payload::Data(payload),
            ttl,
            require_ack,
        )
    }

    pub fn new_ack(
        source_id: Uid,
        destination_id: Option<Uid>,
        payload: AckType,
        ttl: u8,
        require_ack: bool,
    ) -> Self {
        Self::new(
            source_id,
            destination_id,
            Payload::Ack(payload),
            ttl,
            require_ack,
        )
    }

    pub fn new_command(
        source_id: Uid,
        destination_id: Option<Uid>,
        payload: CommandType,
        ttl: u8,
        require_ack: bool,
    ) -> Self {
        Self::new(
            source_id,
            destination_id,
            Payload::Command(payload),
            ttl,
            require_ack,
        )
    }

    pub fn new_route(
        source_id: Uid,
        destination_id: Option<Uid>,
        payload: RouteType,
        ttl: u8,
        require_ack: bool,
    ) -> Self {
        Self::new(
            source_id,
            destination_id,
            Payload::Route(payload),
            ttl,
            require_ack,
        )
    }

    pub fn new_discovery(
        source_id: Uid,
        destination_id: Option<Uid>,
        ttl: u8,
        require_ack: bool,
        device_config: DeviceConfig,
    ) -> Self {
        let discovery_payload = DiscoveryType {
            original_ttl: ttl,
            sender_capabilities: device_config.device_capabilities, // Use the provided config
        };
        Self::new(
            source_id,
            destination_id,
            Payload::Discovery(discovery_payload),
            ttl,
            require_ack,
        )
    }

    pub fn source_id(&self) -> Uid {
        self.source_id
    }

    pub fn message_id(&self) -> u32 {
        self.message_id
    }

    pub fn set_message_id(&mut self, message_id: u32) {
        self.message_id = message_id;
    }

    pub fn req_ack(&self) -> bool {
        self.req_ack
    }

    pub fn destination_id(&self) -> Option<Uid> {
        self.destination_id
    }

    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    pub fn ttl(&self) -> u8 {
        self.ttl
    }

    pub fn decrement_ttl(&mut self) {
        self.ttl = self.ttl.saturating_sub(1);
    }

    pub fn is_expired(&self) -> bool {
        self.ttl == 0
    }

    pub fn is_for_me(&self, uid: Uid) -> bool {
        self.destination_id == Some(uid) || self.destination_id.is_none()
    }
}

impl TryFrom<&mut [u8]> for Message {
    type Error = MessageError;

    fn try_from(data: &mut [u8]) -> Result<Self, Self::Error> {
        postcard::from_bytes_cobs(data).map_err(|_| MessageError::DeserializationError)
    }
}

impl From<Message> for [u8; MAX_MESSAGE_SIZE] {
    fn from(message: Message) -> Self {
        let mut data = [0; MAX_MESSAGE_SIZE];
        let _ = postcard::to_slice_cobs(&message, &mut data);
        data
    }
}
