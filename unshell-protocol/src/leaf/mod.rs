use crate::endpoint::EndpointRef;

pub trait Leaf {
    fn get_name(&self) -> &'static str;
    fn update<'a>(&mut self, _: &mut EndpointRef<'a>);
}
