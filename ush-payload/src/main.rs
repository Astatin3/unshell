#![macro_use]
extern crate unshell;

use unshell::{
    info,
    logger::PrettyLogger,
    tree::{Endpoint, Tree, TreeRequest},
};

struct EndpointTest;

impl Endpoint for EndpointTest {
    fn request(&mut self, request: TreeRequest) -> TreeRequest {
        info!("Got request");
        TreeRequest {
            request_type: request.request_type,
            path: request.path,
            content_type: request.content_type,
            data: request.data,
        }
    }
}

fn main() {
    PrettyLogger::init();

    info!("Initiated");

    let mut tree = Tree::default();

    tree.add_endpoint(EndpointTest, vec!["path1".to_string()]);

    tree.request(TreeRequest {
        path: vec!["path1".to_string(), "path2".to_string()],
        request_type: unshell::tree::TreeRequestType::Read,
        content_type: "TEST".to_string(),
        data: Vec::new(),
    });
}
