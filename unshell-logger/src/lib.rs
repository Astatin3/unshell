use log::{LevelFilter, Log, SetLoggerError};

#[allow(dead_code)]
pub type SetupLogger =
    extern "C" fn(logger: &'static dyn Log, level: LevelFilter) -> Result<(), SetLoggerError>;

#[unsafe(no_mangle)]
pub extern "C" fn setup_logger(
    logger: &'static dyn log::Log,
    level: log::LevelFilter,
) -> Result<(), log::SetLoggerError> {
    log::set_max_level(level);
    log::set_logger(logger)
}
