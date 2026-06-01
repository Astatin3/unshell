use alloc::vec::Vec;

/// Error returned when a packet cannot create a new session.
pub enum SessionInitError {
    /// The packet was intentionally consumed without creating state or sending output.
    Rejected,

    /// The packet was rejected with response data that should be sent on the same hook.
    Response {
        /// Raw `Packet::data` for the response frame.
        data: Vec<u8>,

        /// Whether the response should close the hook after successful routing.
        end_hook: bool,
    },
}

impl SessionInitError {
    /// Creates a silent session rejection.
    pub fn rejected() -> Self {
        Self::Rejected
    }

    /// Creates a non-final response for a rejected session open.
    pub fn response(data: Vec<u8>) -> Self {
        Self::Response {
            data,
            end_hook: false,
        }
    }

    /// Creates a final response for a rejected session open.
    pub fn response_final(data: Vec<u8>) -> Self {
        Self::Response {
            data,
            end_hook: true,
        }
    }
}
