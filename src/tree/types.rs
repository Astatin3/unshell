use alloc::{string::String, vec::Vec};

use crate::obfuscate::sym;

pub const TYPE_NONE: &'static str = sym!("core/None");

pub const TYPE_PROCEDURE_CALL_DESCRIPTOR: &'static str = sym!("core/Procedure_call_descriptor");
pub struct ProcedureCallDescriptor {
    name: String,
}

pub const TYPE_PROCEDURE_CALL_DESCRIPTOR_LIST: &'static str =
    sym!("core/Procedure_call_descriptor_list");
pub type ProcedureCallDescriptorList = Vec<ProcedureCallDescriptor>;

pub const TYPE_PROCEDURE_CALL_ARGUMENTS: &'static str = sym!("core/Procedure_call_arguments");
