// use std::collections::VecDeque;

use alloc::{string::String, vec::Vec};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct TreeRequest {
    // The exact path that this packet should be heading down to
    pub path: Vec<String>,
    // // The list of previous paths that this packet came from
    // // This is the destination path added in reverse order
    // pub source_path: VecDeque<String>,
    pub request_type: TreeRequestType,

    // The data type of the payload, to determine how to deserialize and interpret it on the other side
    // This is equivalent to HTTP's content-type header
    pub content_type: String,

    // The payload of the packet
    pub data: Vec<u8>,
}

#[derive(Archive, Deserialize, Serialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub enum TreeRequestType {
    Return = 0,
    Read = 1,
    Write = 2,
    Submit = 3,

    ListBranches = 10,

    // CreateField = 3,
    // DeleteField = 4,
    UnnamedError = 100,
    NoBranchError = 101,
    ProtocolError = 102,
    ExecutionError = 103,
}
