use crate::protocol::Endpoint;

#[cfg(feature = "interface")]
use crate::{interface::InterfaceStore, protocol::leaf_meta::LeafMeta};

/// Application extension point hosted by an [`Endpoint`].
///
/// A leaf owns product-specific state and reacts to packets that endpoint routing has
/// already delivered locally. The trait intentionally stays small so handwritten
/// leaves, generated leaves, and test leaves can all share the same endpoint loop.
pub trait Leaf {
    /// Returns the stable local identifier for this leaf implementation.
    fn get_id(&self) -> u32;

    /// Advances the leaf by one endpoint update tick.
    ///
    /// Implementations normally drain matching inbound packets, mutate leaf-owned
    /// state, then enqueue outbound packets with [`Endpoint::add_outbound`].
    fn update(&mut self, _: &mut Endpoint);

    /// Advances the leaf while recording caller-owned interface state.
    ///
    /// Plain handwritten leaves can ignore the interface store and reuse their normal
    /// update path. Generated leaves override this to route through the same template
    /// helpers with packet flow logging enabled.
    #[cfg(feature = "interface")]
    fn update_interface(&mut self, endpoint: &mut Endpoint, _: &mut InterfaceStore) {
        self.update(endpoint);
    }

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta::anonymous()
    }

    #[cfg(feature = "interface_ratatui")]
    fn render_ratatui(
        &mut self,
        _: &mut ratatui::Frame<'_>,
        _: ratatui::layout::Rect,
        _: &mut InterfaceStore,
    ) {
    }
}
