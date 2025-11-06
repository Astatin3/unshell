#[macro_export]
macro_rules! module_interface {
    ($interface_name:ident { $(fn $fn_name:ident($($arg:ident : $ty:ty),* $(,)?) $(-> $ret:ty)?);* $(;)? }) => {
        #[repr(C)]
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy)]
        pub struct $interface_name {
            $(
                $fn_name: extern "C" fn($($ty),*) $(-> $ret)?,
            )*
        }

        impl $interface_name {
            /// Create from raw function pointers
            ///
            /// # Safety
            ///
            /// The caller must ensure all function pointers are valid and have
            /// the correct signatures
            pub fn from_raw(
                $($fn_name: extern "C" fn($($ty),*) $(-> $ret)?),*
            ) -> Self {
                Self {
                    $($fn_name),*
                }
            }

            $(
                #[inline(always)]
                pub fn $fn_name(&self, $($arg: $ty),*) $(-> $ret)? {
                    (self.$fn_name)($($arg),*)
                }
            )*
        }
    };
}
// mod callable;

// pub enum Error {
//     LibLoadingError(libloading::Error),
// }

// #[cfg(feature = "exec")]
// pub struct Module {
//     lib: libloading::Library,
// }

// impl Module {
//     pub fn new(path: &str) -> Result<Self, Error> {
//         let lib =
//             unsafe { libloading::Library::new(path) }.map_err(|e| Error::LibLoadingError(e))?;
//         Ok(Module { lib })
//     }
//     pub fn functions(&self) {
//         // self.lib.

//         // self.lib.get(name).map_err(|e| Error::LibLoadingError(e))
//     }
// }

// pub use callable::Callable;
// pub use callable::Function;
// pub use callable::wrap;
