//! Caller-owned interface state for UI frontends.
//!
//! Protocol leaves stay headless. When a UI wants packet flow, timing, or render
//! state, it passes an [`InterfaceStore`] through the feature-gated interface path.

mod event;
mod key;
mod store;
mod target;
mod view;

pub use event::{InterfaceEvent, InterfaceEventKind};
pub use key::{ProcedureKey, SessionKey};
pub use store::InterfaceStore;
pub use view::{ProcedureView, SessionView, SessionViewStatus};

pub(crate) use target::InterfaceTarget;
