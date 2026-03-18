use alloc::{boxed::Box, string::String, vec::Vec};

mod request;

pub use request::{TreeRequest, TreeRequestType};

pub mod types;

#[derive(Default)]
pub struct Tree {
    endpoints: Vec<(Box<dyn Endpoint>, Vec<String>)>,
}

impl Tree {
    pub fn add_endpoint<T: Endpoint + 'static>(&mut self, endpoint: T, path: Vec<String>) {
        self.add_endpoint_box(Box::new(endpoint), path);
    }
    pub fn add_endpoint_box(&mut self, endpoint: Box<dyn Endpoint>, path: Vec<String>) {
        self.endpoints.push((endpoint, path));
    }

    pub fn get_endpoint(&mut self, search_path: &Vec<String>) -> Option<&mut Box<dyn Endpoint>> {
        for (endpoint, endpoint_path) in &mut self.endpoints {
            if search_path.len() < endpoint_path.len() {
                return None;
            }

            for i in 0..endpoint_path.len() {
                if search_path[i] != endpoint_path[i] {
                    return None;
                }
            }

            return Some(endpoint);
        }

        return None;
    }

    pub fn request(&mut self, request: TreeRequest) -> TreeRequest {
        if let Some(endpoint) = self.get_endpoint(&request.path) {
            endpoint.request(request)
        } else {
            TreeRequest {
                path: request.path,
                request_type: TreeRequestType::NoBranchError,
                content_type: types::TYPE_NONE.into(),
                data: Vec::with_capacity(0),
            }
        }
    }
}

pub trait Endpoint {
    fn request(&mut self, request: TreeRequest) -> TreeRequest;
}
