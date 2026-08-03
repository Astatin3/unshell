use crate::{HookID, NodeID, ProcedureID};

#[derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize, Debug, PartialEq)]
pub struct PacketHeader {
    /// Determines the type of packet
    /// bit 0-1: packet type, 0: Downwards, 1: upwards,
    /// bit 3: close bit, if this is set, then the hook ID will be invalidated.
    /// bit 4-7: unused
    pub header: u8,

    /// A number that's update for every relay of this packet
    /// Useful for detection of cycles and getting the correct
    /// element out of the path.
    ///
    /// The protocol is limited to 255 hops
    ///
    /// Downwards:
    /// Corresponds to the index out of the path.
    /// It gets initialized 0,
    /// incremented every relay
    ///
    /// Upwards:
    /// This is initiated at the distance between the child and the target.
    /// Decremented every relay.
    pub depth_number: u8,

    /// Downwards:
    /// The explicit path
    /// First element is the root node, last element is the destination
    ///
    /// Upwards:
    /// A packet route trace
    pub path: Vec<NodeID>,

    /// Hook ID
    pub hook_id: HookID,

    /// Procedure ID
    pub procedure_id: ProcedureID,
}

impl PacketHeader {
    const PACKET_TYPE_MASK: u8 = 0b0000_0011;
    const CLOSE_BIT: u8 = 0b0000_1000;

    const PACKET_TYPE_DOWNWARDS: u8 = 0;
    const PACKET_TYPE_UPWARDS: u8 = 1;

    pub fn is_downwards(&self) -> bool {
        self.header & Self::PACKET_TYPE_MASK == Self::PACKET_TYPE_DOWNWARDS
    }

    pub fn is_upwards(&self) -> bool {
        self.header & Self::PACKET_TYPE_MASK == Self::PACKET_TYPE_UPWARDS
    }

    pub fn is_close(&self) -> bool {
        self.header & Self::CLOSE_BIT != 0
    }
}
