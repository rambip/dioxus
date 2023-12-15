use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use std::sync::{Arc, OnceLock};

use crate::{
    error::{self, ValueDroppedError},
    references::{
        GenerationalRef, GenerationalRefBorrowInfo, GenerationalRefMut,
        GenerationalRefMutBorrowInfo,
    },
    AnyStorage, Mappable, MappableMut, MemoryLocation, MemoryLocationInner, Storage,
};

/// A thread safe storage. This is slower than the unsync storage, but allows you to share the value between threads.
#[derive(Default)]
pub struct SyncStorage(RwLock<Option<Box<dyn std::any::Any + Send + Sync>>>);

static SYNC_RUNTIME: OnceLock<Arc<Mutex<Vec<MemoryLocation<SyncStorage>>>>> = OnceLock::new();

fn sync_runtime() -> &'static Arc<Mutex<Vec<MemoryLocation<SyncStorage>>>> {
    SYNC_RUNTIME.get_or_init(|| Arc::new(Mutex::new(Vec::new())))
}

impl AnyStorage for SyncStorage {
    fn data_ptr(&self) -> *const () {
        self.0.data_ptr() as *const ()
    }

    fn take(&self) -> bool {
        self.0.write().take().is_some()
    }

    fn claim() -> MemoryLocation<Self> {
        sync_runtime().lock().pop().unwrap_or_else(|| {
            let data: &'static MemoryLocationInner<Self> =
                &*Box::leak(Box::new(MemoryLocationInner {
                    data: Self::default(),
                    #[cfg(any(debug_assertions, feature = "check_generation"))]
                    generation: 0.into(),
                    #[cfg(any(debug_assertions, feature = "debug_borrows"))]
                    borrow: Default::default(),
                }));
            MemoryLocation(data)
        })
    }

    fn recycle(location: &MemoryLocation<Self>) {
        location.drop();
        sync_runtime().lock().push(*location);
    }
}

impl<T> Mappable<T> for MappedRwLockReadGuard<'static, T> {
    type Mapped<U: 'static> = MappedRwLockReadGuard<'static, U>;

    fn map<U: 'static>(_self: Self, f: impl FnOnce(&T) -> &U) -> Self::Mapped<U> {
        MappedRwLockReadGuard::map(_self, f)
    }

    fn try_map<U: 'static>(
        _self: Self,
        f: impl FnOnce(&T) -> Option<&U>,
    ) -> Option<Self::Mapped<U>> {
        MappedRwLockReadGuard::try_map(_self, f).ok()
    }
}

impl<T> MappableMut<T> for MappedRwLockWriteGuard<'static, T> {
    type Mapped<U: 'static> = MappedRwLockWriteGuard<'static, U>;

    fn map<U: 'static>(_self: Self, f: impl FnOnce(&mut T) -> &mut U) -> Self::Mapped<U> {
        MappedRwLockWriteGuard::map(_self, f)
    }

    fn try_map<U: 'static>(
        _self: Self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Option<Self::Mapped<U>> {
        MappedRwLockWriteGuard::try_map(_self, f).ok()
    }
}

impl<T: Sync + Send + 'static> Storage<T> for SyncStorage {
    type Ref = GenerationalRef<T, MappedRwLockReadGuard<'static, T>>;
    type Mut = GenerationalRefMut<T, MappedRwLockWriteGuard<'static, T>>;

    fn try_read(
        &'static self,
        #[cfg(any(debug_assertions, feature = "debug_ownership"))]
        created_at: &'static std::panic::Location<'static>,
        #[cfg(any(debug_assertions, feature = "debug_ownership"))] at: GenerationalRefBorrowInfo,
    ) -> Result<Self::Ref, error::BorrowError> {
        RwLockReadGuard::try_map(self.0.read(), |any| any.as_ref()?.downcast_ref())
            .map_err(|_| {
                error::BorrowError::Dropped(ValueDroppedError {
                    #[cfg(any(debug_assertions, feature = "debug_ownership"))]
                    created_at,
                })
            })
            .map(|guard| {
                GenerationalRef::new(
                    guard,
                    #[cfg(any(debug_assertions, feature = "debug_ownership"))]
                    at,
                )
            })
    }

    fn try_write(
        &'static self,
        #[cfg(any(debug_assertions, feature = "debug_ownership"))]
        created_at: &'static std::panic::Location<'static>,
        #[cfg(any(debug_assertions, feature = "debug_ownership"))] at: GenerationalRefMutBorrowInfo,
    ) -> Result<Self::Mut, error::BorrowMutError> {
        RwLockWriteGuard::try_map(self.0.write(), |any| any.as_mut()?.downcast_mut())
            .map_err(|_| {
                error::BorrowMutError::Dropped(ValueDroppedError {
                    #[cfg(any(debug_assertions, feature = "debug_ownership"))]
                    created_at,
                })
            })
            .map(|guard| {
                GenerationalRefMut::new(
                    guard,
                    #[cfg(any(debug_assertions, feature = "debug_ownership"))]
                    at,
                )
            })
    }

    fn set(&self, value: T) {
        *self.0.write() = Some(Box::new(value));
    }
}
