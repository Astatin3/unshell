#[derive(Debug)]
pub enum EndpointError {
    NoAbsoultePathYet,
    IncorrectAbsolutePath,

    RouteNotExist,
    HookDuplicate,
    HookNotExist,
}
