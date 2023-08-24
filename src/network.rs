mod routing_table;
mod mesh_error;
mod route;

use embedded_hal_async::delay::DelayUs;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_traits::RadioKind;
use crate::device::LoraDevice;
use crate::message::Message;
use crate::network::mesh_error::MeshError;
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
