pub trait Router {
    type HeaderType;

    fn route<F, G, H>(
        &mut self,
        packet: Self::HeaderType,

        // Called when a packet must be immediately written
        // to some stream
        callback_relay: F,

        // Called when a packet should be processed
        callback_recv: G,

        // Called when a packet is malformed,
        // it's packet data should be cleared
        callback_malformed: H,
    ) where
        F: FnMut(Self::HeaderType),
        G: FnMut(Self::HeaderType),
        H: FnMut();
}
