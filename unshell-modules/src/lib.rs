#[macro_use]
extern crate log;

mod logger;
mod manager;
mod module;

pub use logger::setup_logger;
pub use manager::Manager;
pub use module::Module;

///Generic error type for module-related operations.
#[derive(Debug)]
pub enum ModuleError {
    LibLoadingError(libloading::Error),
    LogError(log::SetLoggerError),
    LinkError(String),
    Error(String),
}

/// Trait for defining modules that have a runtime.
pub trait ModuleRuntime: Send {
    /// Returns true if the module is running.
    /// After returning false, the module will be dropped.
    fn is_running(&self) -> bool;
    /// Consumes the module, implementation should kill whatever is running.
    fn kill(self: Box<Self>);
}

/// "Module Interface" helper macro that creates a struct with function pointers
/// Useful for defining and requiring modules' functions accross FFI boundry.
#[macro_export]
macro_rules! module_interface {
    ($(#[$struct_meta:meta])* $interface_name:ident { $($(#[$fn_meta:meta])* fn $fn_name:ident $(<$($gen:ident),+ $(,)?>)?($($arg:ident : $ty:ty),* $(,)?) $(-> $ret:ty)? $(where $($where_clause:tt)*)?);* $(;)? }) => {

        #[repr(C)]
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy)]
        #[allow(improper_ctypes_definitions)]
        $(#[$struct_meta])*
        pub struct $interface_name {
            $(
                // This line will FAIL TO COMPILE if you use generics in the macro input.
                // You MUST use concrete types like *mut c_void for "generic" data.
                $fn_name: extern "C" fn($($ty),*) $(-> $ret)?,
            )*
        }

        impl $interface_name {
            $(
                #[inline(always)]
                $(#[$fn_meta])* // Propagate function attributes
                // This is the fix for the `impl` block.
                // It adds the captured generics and where-clause to the wrapper function.
                pub fn $fn_name $(<$($gen),+>)? (&self, $($arg: $ty),*) $(-> $ret)?
                $(where $($where_clause)*)?
                {
                    (self.$fn_name)($($arg),*)
                }
            )*

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
        }
    };
}
