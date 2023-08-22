mod routing_table;
mod mesh_error;
mod route;

use embedded_hal_async::delay::DelayUs;
use lora_phy::mod_traits::RadioKind;
use crate::device::LoraDevice;
use crate::network::routing_table::RoutingTable;

pub struct MeshNetwork<RK, DLY>
    where
        RK: RadioKind,
        DLY: DelayUs,
{
    device: LoraDevice<RK, DLY>,
    routing_table: RoutingTable,
}
