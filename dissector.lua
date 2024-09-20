multilink_proto = Proto("mllink","Multilink")

local packet_type_str = {
    [0] = "Keepalive",
    [1] = "Data",
    [2] = "DataDup",
    [3] = "Initiate"
}

function multilink_proto.dissector(buffer,pinfo,tree)
    pinfo.cols.protocol = "SEAMLESS"
    local subtree = tree:add(multilink_proto,buffer(),"Multilink Protocol Data")
    subtree:add(buffer(0,1 ),"C: " ..                   buffer(0,1):uint())
    subtree:add(buffer(1,1 ),"N: " ..                   buffer(1,1):uint())
    subtree:add(buffer(2,1 ),"I: " ..                   buffer(2,1):uint())
    subtree:add(buffer(3,2 ),"Length: " ..              buffer(3,2):le_uint())
    subtree:add(buffer(5,1 ),"Stream ID: " ..           buffer(5,1):uint())
    subtree:add(buffer(6,4 ),"Stream Sequence: " ..     buffer(6,4):le_uint())
    subtree:add(buffer(10,2),"Timestamp: " ..           buffer(10,2):le_uint())    
    subtree:add(buffer(12,2),"Timestamp Reply: " ..     buffer(12,2):le_uint())
    subtree:add(buffer(14,4),"Link Sequence: " ..       buffer(14,4):le_uint())
    subtree:add(buffer(18,1),"Link Id: " ..             buffer(18,1):uint())

    local packet_type = bit.band(bit.rshift(buffer(19,1):uint(), 0), 0x03)
    local packet_type_str_value = packet_type_str[packet_type] or "Unknown"
    subtree:add(buffer(19,1),"Packet Type: " .. packet_type_str_value)    

    subtree:add(buffer(19,1),"Reorder: " ..             bit.band(bit.rshift(buffer(19,1):le_uint(), 2), 0x01))
    subtree:add(buffer(19,1),"SRRT Calculation: " ..    bit.band(bit.rshift(buffer(19,1):le_uint(), 3), 0x01))
    subtree:add(buffer(19,1),"Padding: " ..             bit.band(bit.rshift(buffer(19,1):le_uint(), 4), 0x0F))
end
-- load the udp.port table
udp_table = DissectorTable.get("udp.port")
-- register our protocol to handle udp port 6001
udp_table:add(6001,multilink_proto)

