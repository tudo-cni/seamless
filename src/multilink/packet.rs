use modular_bitfield::prelude::*;

#[derive(BitfieldSpecifier, Debug)]
#[bits = 2]
pub enum PacketType {
    /// The `Keepalive` packets are send in regular time intervals to check for connectivity and link quality.
    Keepalive,
    /// The `Data` packets are used to encapsulate higher higher level ip packets to send them using multilink
    Data,
    /// The `DataDup` packets are used when a redundant data transmission is requested. In this case the data is duplicated and send over multiple links.
    DataDup,
    Initiate,
}

#[bitfield]
#[derive(Debug)]
#[allow(dead_code)]
pub struct MultiLinkHeader {
    /// Just.
    #[skip(getters)]
    pub c: u8, //  0-7
    /// for.
    #[skip(getters)]
    pub n: u8, //  8-15
    /// fun.
    #[skip(getters)]
    pub i: u8, //  16-23
    /// The length of the packet represented as a `u16`. Containing both multilink header and payload.
    #[skip(getters)]
    pub len: u16, //  24-39
    /// The unique id of the current higher level data stream as a `u8`. Obtained from corresponding application object. Otherwise `0`.
    pub stream_id: u8, //  40-47
    /// The current stream packet id of the current higher level data stream as a `u32`. Obtained from corresponding application object. Otherwise equal to the general scheduling sequence number.
    pub stream_seq: u32, //  48-79
    /// The current timestamp as a wrapping `u16` used for round trip time calculation.
    pub timestamp: u16, // 80-95
    /// The kept timestamp from the last packet received of the equivalent link with offset accounting for the time the packet was kept back. Used for round trip time calculation.
    pub timestamp_reply: u16, //  96-111
    /// The current link sequence as a wrapping `u32` used for packet loss calculation.
    pub link_seq: u32, //  112-143
    /// The current link id as a u8 used for link down notification.
    pub link_id: u8, //  144-151
    /// The type of packet defined by the `PacketType` enum.
    pub pkt_type: PacketType, //  152-153
    /// Define if a packet should be considered for reordering. For realtime applications reordering might not be suitable.
    pub reorder: bool, //  154-154
    /// Define if a packet should be used for round trip time calculation. Might be cases in which this should not be done. Tell me if you find one...
    pub srrt_calculation: bool, //  155
    #[skip(setters, getters)]
    pub padding: B4, //  156-159
} // 160 Bit overall --> 20 Byte!

pub const MULTILINK_HEADER_SIZE: usize = MultiLinkHeader::new().into_bytes().len();

fn build_base_multilink_header() -> MultiLinkHeader {
    MultiLinkHeader::new()
        .with_c(0x43)
        .with_n(0x4E)
        .with_i(0x49)
        .with_stream_id(255)
        .with_stream_seq(0)
        .with_reorder(false)
        .with_srrt_calculation(false)
        .with_timestamp(0)
        .with_timestamp_reply(0)
        .with_link_id(0)
        .with_link_seq(0)
}

#[allow(clippy::too_many_arguments)]
pub fn build_multilink_header(
    pkt_type: PacketType,
    stream_id: u8,
    stream_seq: u32,
    reorder: bool,
    payload_size: u16,
    timestamp: u16,
    calc_rtt: bool,
    timestamp_reply: u16,
) -> MultiLinkHeader {
    let mut header = build_base_multilink_header();
    header.set_len(MULTILINK_HEADER_SIZE as u16 + payload_size);
    header.set_pkt_type(pkt_type);
    header.set_stream_id(stream_id);
    header.set_stream_seq(stream_seq);
    header.set_reorder(reorder);
    header.set_srrt_calculation(calc_rtt);
    header.set_timestamp(timestamp);
    header.set_timestamp_reply(timestamp_reply);
    header
}

/// Building a `Keepalive` multilink header.
pub fn build_keepalive_multilink_header(
    timestamp: u16,
    calc_rtt: bool,
    timestamp_reply: u16,
    link_seq: u32,
) -> MultiLinkHeader {
    let mut header = build_base_multilink_header();
    header.set_len(MULTILINK_HEADER_SIZE as u16);
    header.set_pkt_type(PacketType::Keepalive);
    header.set_stream_id(255);
    header.set_stream_seq(0);
    header.set_reorder(false);
    header.set_srrt_calculation(calc_rtt);
    header.set_timestamp(timestamp);
    header.set_timestamp_reply(timestamp_reply);
    header.set_link_id(0);
    header.set_link_seq(link_seq);
    header
}

/// Building a `Keepalive` multilink header.
pub fn build_initiate_multilink_header() -> MultiLinkHeader {
    let mut header = build_base_multilink_header();
    header.set_len(MULTILINK_HEADER_SIZE as u16);
    header.set_pkt_type(PacketType::Initiate);
    header
}
