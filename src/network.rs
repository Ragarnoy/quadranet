mod routing_table;
mod mesh_error;
mod route;

use embedded_hal_async::delay::DelayUs;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_traits::RadioKind;
use crate::device::LoraDevice;
use crate::message::Message;
use crate::network::mesh_error::MeshError;
use crate::network::route::Route;
use crate::network::routing_table::RoutingTable;

pub struct MeshNetwork<RK, DLY>
    where
        RK: RadioKind,
        DLY: DelayUs,
{
    device: LoraDevice<RK, DLY>,
    routing_table: RoutingTable,
}

impl <RK, DLY> MeshNetwork<RK, DLY>
    where
        RK: RadioKind,
        DLY: DelayUs,
{
    pub fn new(device: LoraDevice<RK, DLY>, routing_table: RoutingTable) -> Self {
        Self {
            device,
            routing_table,
        }
    }

    pub async fn discover_nodes(&mut self, depth: u16) -> Result<(), RadioError> {
        let message = Message::new_discovery(self.device.uid, depth);
        self.device.send_message(message).await
    }

    pub async fn start_discovery(&mut self) -> Result<(), RadioError> {
        self.discover_nodes(0).await
    }

    pub async fn receive_message(&mut self) -> Result<Message, MeshError> {
        let mut buf = [0u8; 74];  // Buffer to hold incoming message
        let (rx_length, _packet_status) = self.device.receive_message(&mut buf).await.map_err(|source| MeshError::DeviceError { source })?;

        // Deserialize the received message
        let received_message = Message::try_from(&buf[0..rx_length as usize])
            .map_err(|e| MeshError::MessageError { source: e })?;

        // Update the routing table
        self.routing_table.update(received_message.sender_uid.get(), Route { next_hop: received_message.sender_uid });

        // Check if the message is for this node or needs to be forwarded
        if let Some(receiver_uid) = received_message.receiver_uid {
            if receiver_uid.get() == self.device.uid.get() {
                // Process the message
                // ...
            } else {
                // Forward the message to the next hop
                self.send_message(received_message.clone()).await?;
            }
        }

        Ok(received_message)
    }

    pub async fn send_message(&mut self, mut message: Message) -> Result<(), MeshError> {
        // Look up the routing table to find the next hop
        let route = self.routing_table.lookup_route(message.receiver_uid.unwrap().get())
            .ok_or(MeshError::RouteNotFound)?;

        // Update the next_hop in the message and send it
        message.next_hop = Some(route.next_hop);
        self.device.send_message(message).await.map_err(|e| MeshError::DeviceError { source: e.into() } )?;

        Ok(())
    }

}
