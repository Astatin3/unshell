//! # Info Module
//!
//! Provides basic system information about the target at `/info`.
//!
//! ## Supported requests
//!
//! | Path | RequestType | Returns |
//! |---|---|---|
//! | `/info` | `Read` | UTF-8 string: OS name, arch, hostname |
//! | `/info` | `GetProcedures` | List of available procedures |
//!
//! ## Example
//!
//! From the operator CLI:
//! ```text
//! unshell [agents/abc123]> read info
//! linux x86_64 hostname=target-machine
//! ```

use unshell::protocol::{
    content, ProcedureDescriptor, RequestType, ResponseStatus, TreeRequest, TreeResponse,
};
use unshell::tree::Endpoint;

/// Returns basic system information about the target host.
pub struct InfoModule;

impl Endpoint for InfoModule {
    fn handle(&mut self, request: TreeRequest) -> TreeResponse {
        match request.request_type {
            RequestType::Read => handle_read(request),
            RequestType::GetProcedures => handle_get_procedures(request),
            _ => TreeResponse {
                request_id: request.request_id,
                status: ResponseStatus::UnsupportedOperation,
                content_type: content::NONE.to_owned(),
                data: Vec::new(),
            },
        }
    }
}

/// Return a one-line system summary.
fn handle_read(request: TreeRequest) -> TreeResponse {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let hostname = hostname();
    let info = format!("os={os} arch={arch} hostname={hostname}");
    TreeResponse {
        request_id: request.request_id,
        status: ResponseStatus::Ok,
        content_type: content::UTF8_STRING.to_owned(),
        data: info.into_bytes(),
    }
}

/// Return a list of procedures this module supports.
fn handle_get_procedures(request: TreeRequest) -> TreeResponse {
    let procedures = vec![ProcedureDescriptor {
        name: "read".to_owned(),
        description: "Returns os, arch, and hostname of this target".to_owned(),
    }];

    let Ok(payload) = rkyv::to_bytes::<rkyv::rancor::Error>(&procedures) else {
        return TreeResponse {
            request_id: request.request_id,
            status: ResponseStatus::ExecutionError,
            content_type: content::NONE.to_owned(),
            data: Vec::new(),
        };
    };

    TreeResponse {
        request_id: request.request_id,
        status: ResponseStatus::Ok,
        content_type: content::PROCEDURE_LIST.to_owned(),
        data: payload.to_vec(),
    }
}

/// Get the system hostname, or "unknown" if unavailable.
fn hostname() -> String {
    // std::net::IpAddr doesn't give us hostname; use /etc/hostname or gethostname
    // For now, use a simple approach that doesn't require extra deps.
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|_| "unknown".to_owned())
}
