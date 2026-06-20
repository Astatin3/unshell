use crate::protocol::{
    Endpoint, Packet, Session, SessionEntry, SessionFamily, SessionInitError, SessionStatus,
};

/// Dispatches one packet into a generated session family.
///
/// The macro picks `S` and the family field. This helper owns the boring details:
/// find the hook, initialize missing sessions, and route rejected responses. The
/// interface build uses the sibling logging helper so the smallest endpoint binary
/// does not mention the interface logging types on its hot update path.
pub fn dispatch_session<L, S>(
    endpoint: &mut Endpoint,
    leaf: &mut L,
    family: &mut SessionFamily<S>,
    packet: Packet,
) where
    S: Session<L>,
{
    let hook_id = packet.hook_id;
    let procedure_id = S::PROCEDURE_ID;
    if let Some(entry) = family
        .entries
        .iter_mut()
        .find(|entry| entry.hook_id == hook_id)
    {
        entry.inbox.push(packet);
        return;
    }

    let Ok(path) = endpoint.hook_path(hook_id) else {
        return;
    };
    match S::init(leaf, packet) {
        Ok(state) => {
            family.entries.push(SessionEntry::new(hook_id, state));
        }
        Err(SessionInitError::Rejected) => {}
        Err(SessionInitError::Response { data, end_hook }) => {
            let packet = Packet {
                hook_id,
                end_hook,
                path,
                procedure_id,
                data,
            };

            let _ = endpoint.add_outbound(packet);
        }
    }
}

/// Updates every live session in one generated session family.
pub fn update_session_family<L, S>(
    endpoint: &mut Endpoint,
    leaf: &mut L,
    family: &mut SessionFamily<S>,
) where
    S: Session<L>,
{
    for entry in &mut family.entries {
        if entry.closed {
            continue;
        }

        let status = S::update(leaf, &mut entry.state, &mut entry.inbox, endpoint);

        if matches!(status, SessionStatus::Closed) {
            entry.closed = true;
        }
    }

    family.entries.retain(|entry| !entry.closed);
}
