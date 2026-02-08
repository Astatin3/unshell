pub enum TreeMessage {
    Request,
    Response,
    Result(crate::error::ModuleError),
}

pub enum TreeType {
    RootTree,

    TypeA,
    TypeB,
    TypeC,
}
