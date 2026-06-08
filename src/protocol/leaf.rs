use crate::protocol::Endpoint;

#[cfg(feature = "interface")]
use crate::protocol::leaf_meta::LeafMeta;

#[cfg(feature = "interface_ratatui")]
use crate::interface::InterfaceContext;

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

    #[cfg(feature = "interface")]
    fn get_meta(&self) -> LeafMeta {
        LeafMeta::anonymous()
    }

    /// Runs one Ratatui interface pass for this leaf.
    ///
    /// This is the only public UI lifecycle method. The default keeps handwritten
    /// leaves usable by advancing normal protocol state and drawing shared leaf chrome
    /// with no active or historical sessions. Generated leaves override it to serialize
    /// their session objects into the context database, load historical session blobs,
    /// and render through their known session types.
    #[cfg(feature = "interface_ratatui")]
    fn update_interface_ratatui(
        &mut self,
        endpoint: &mut Endpoint,
        ctx: &mut InterfaceContext<'_>,
        frame: &mut ratatui::Frame<'_>,
        area: ratatui::layout::Rect,
    ) {
        self.update(endpoint);

        let meta = self.get_meta();
        let _ = ctx.ratatui.render_leaf_chrome(&meta, 0, 0, frame, area);
    }
}
