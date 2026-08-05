use crate::api::{
    ProcedureId,
    procedure::ErasedProcedure,
    procedure_table::{ProcedureManifest, ProcedureTable},
};

/// Compile-time table. Intended to be built by a macro, e.g.:
///
/// ```ignore
/// static_procedures! {
///     EchoProcedure,
///     AuthProcedure,
///     FileTransferProcedure,   // gate with #[cfg(feature = "fs")]
/// }
/// ```
///
/// expanding to a `phf::Map<u32, &'static dyn ErasedProcedure>` built at
/// compile time: lookup is a perfect hash, no runtime insertion cost,
/// no lock. Binary-size control (leaving a procedure out entirely, not
/// just making it unreachable) comes from Cargo features gating which
/// types the macro invocation lists -- the map itself doesn't do dead
/// code elimination on its own entries, since everything referenced in
/// a `const` map is linked in.
pub struct StaticProcedureTable {
    map: phf::Map<u32, &'static dyn ErasedProcedure>,
}

impl ProcedureTable for StaticProcedureTable {
    fn lookup(&self, id: ProcedureId) -> Option<&dyn ErasedProcedure> {
        self.map.get(&id.0).map(|v| *v)
    }

    fn manifest(&self) -> ProcedureManifest {
        ProcedureManifest {
            ids: self.map.keys().map(|id| ProcedureId(*id)).collect(),
        }
    }
}
