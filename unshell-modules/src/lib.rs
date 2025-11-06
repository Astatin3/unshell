#[macro_use]
extern crate log;

mod logger;
mod module;

pub use logger::setup_logger;
pub use module::Module;

#[derive(Debug)]
pub enum ModuleError {
    LibLoadingError(libloading::Error),
    LinkError(String),
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

module_interface! {
    ManagerInterface {
        fn test123();
    }
}
