#[macro_export]
macro_rules! module {
    ($module_name:ident { $(fn $fn_name:ident($($arg:ident : $ty:ty),* $(,)?) $(-> $ret:ty)?);* $(;)? }) => {
        #[allow(non_camel_case_types)]
        pub struct $module_name;

        impl $module_name {
            $(
                #[inline(always)]
                pub fn $fn_name(&self, $($arg: $ty),*) $(-> $ret)? {
                    $fn_name($($arg),*)
                }
            )*

            /// Create a new instance of this module
            pub fn new() -> Self {
                Self
            }
        }

        impl Default for $module_name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

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
            /// Unsafe cast from a module type to this interface
            ///
            /// # Safety
            ///
            /// The caller must ensure that:
            /// - The module has exactly the same function signatures in the same order
            /// - The functions follow the C calling convention
            /// - The module's memory layout matches this interface
            pub unsafe fn from_module<T>(module: &T) -> Self {
                *(module as *const T as *const Self)
            }

            /// Create from raw function pointers
            ///
            /// # Safety
            ///
            /// The caller must ensure all function pointers are valid and have
            /// the correct signatures
            pub unsafe fn from_raw(
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
