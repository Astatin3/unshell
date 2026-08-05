// ===========================================================================
// 6. Concurrency strategy
// ===========================================================================

/// Marker + associated-wrapper-type trait so `Node<C, T>`'s internals
/// (session store, procedure table access) can be written once and
/// parameterized over how interior mutability is achieved, instead of
/// duplicating the whole struct for each threading model.
pub trait Concurrency {
    type Cell<V>;

    type Ref<'a, V: 'a>: std::ops::Deref<Target = V>;
    type Mut<'a, V: 'a>: std::ops::DerefMut<Target = V>;

    fn new_cell<V>(value: V) -> Self::Cell<V>;
    fn borrow<'a, V>(cell: &'a Self::Cell<V>) -> Self::Ref<'a, V>;
    fn borrow_mut<'a, V>(cell: &'a Self::Cell<V>) -> Self::Mut<'a, V>;
}

pub struct Local;
pub struct Shared;

impl Concurrency for Local {
    type Cell<V> = std::cell::RefCell<V>;
    type Ref<'a, V: 'a> = std::cell::Ref<'a, V>;
    type Mut<'a, V: 'a> = std::cell::RefMut<'a, V>;

    fn new_cell<V>(value: V) -> Self::Cell<V> {
        std::cell::RefCell::new(value)
    }
    fn borrow<'a, V>(cell: &'a Self::Cell<V>) -> Self::Ref<'a, V> {
        cell.borrow()
    }
    fn borrow_mut<'a, V>(cell: &'a Self::Cell<V>) -> Self::Mut<'a, V> {
        cell.borrow_mut()
    }
}

impl Concurrency for Shared {
    type Cell<V> = std::sync::RwLock<V>;
    type Ref<'a, V: 'a> = std::sync::RwLockReadGuard<'a, V>;
    type Mut<'a, V: 'a> = std::sync::RwLockWriteGuard<'a, V>;

    fn new_cell<V>(value: V) -> Self::Cell<V> {
        std::sync::RwLock::new(value)
    }
    fn borrow<'a, V>(cell: &'a Self::Cell<V>) -> Self::Ref<'a, V> {
        cell.read().expect("shared lock poisoned")
    }
    fn borrow_mut<'a, V>(cell: &'a Self::Cell<V>) -> Self::Mut<'a, V> {
        cell.write().expect("shared lock poisoned")
    }
}
