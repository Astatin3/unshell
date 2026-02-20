// Macros that are used that just drop the inside variables
#[macro_export]
macro_rules! log {
    ($level:expr, $fmt:tt) => {{}};
    ($level:expr, $fmt:tt, $($arg:expr),*) => {{}};
}
