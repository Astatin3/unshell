use alloc::vec::Vec;

pub struct LeafMeta {
    pub name: &'static str,
    pub identifier: &'static str,
    pub version: &'static str,
    pub authors: Vec<&'static str>,
}
