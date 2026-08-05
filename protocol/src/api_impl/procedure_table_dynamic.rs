use std::collections::HashMap;

use crate::api::{
    ProcedureId,
    node::NodeError,
    procedure::{ErasedProcedure, Procedure},
    procedure_table::{DynamicRegistry, ProcedureManifest, ProcedureTable},
};

/// Runtime table: procedures can be registered/unregistered after the
/// binary is running (plugin loading, feature negotiation with a
/// remote peer, etc).
pub struct DynamicProcedureTable {
    map: HashMap<ProcedureId, Box<dyn ErasedProcedure>>,
}

impl ProcedureTable for DynamicProcedureTable {
    fn lookup(&self, id: ProcedureId) -> Option<&dyn ErasedProcedure> {
        self.map.get(&id).map(|v| &**v)
    }

    fn manifest(&self) -> ProcedureManifest {
        ProcedureManifest {
            ids: self.map.keys().map(|id| *id).collect(),
        }
    }
}

impl DynamicRegistry for DynamicProcedureTable {
    fn register<P: Procedure + 'static>(&mut self, proc: P) -> Result<(), NodeError> {
        let boxed = Box::new(proc) as Box<dyn ErasedProcedure>;
        self.map.insert(P::ID, boxed);

        Ok(())
    }

    fn register_boxed(
        &mut self,
        id: ProcedureId,
        proc: Box<dyn ErasedProcedure>,
    ) -> Result<(), NodeError> {
        self.map.insert(id, proc);

        Ok(())
    }

    fn unregister(&mut self, id: ProcedureId) -> Option<Box<dyn ErasedProcedure>> {
        self.map.remove(&id)
    }
}
