use crate::endpoint::Endpoint;

pub trait Leaf {
    // Identifier for this leaf
    fn get_id(&self) -> u32;

    // Gets called every program loop
    fn update(&mut self, _: &mut Endpoint);
}
