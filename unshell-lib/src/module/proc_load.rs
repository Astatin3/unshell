// Load a shared object by saving bytes to a filesystem in /proc

use std::{error::Error, ffi::CString, io}; // 0.8

use libloading::Library;

use crate::debug;

// The `memfd_create` syscall flags (MFD_CLOEXEC is common and good practice)
const MFD_CLOEXEC: u32 = 0x0001;
const MFD_ALLOW_SEALING: u32 = 0x0002;

pub fn memfd_create_dlopen(payload: &[u8]) -> Result<Library, Box<dyn Error>> {
    use rand::distr::{Alphanumeric, SampleString};

    let string = Alphanumeric.sample_string(&mut rand::rng(), 16);

    // 1. Create the anonymous in-memory file descriptor using the raw syscall
    let c_name = CString::new(string).expect("CString conversion failed");

    let fd = unsafe { libc::memfd_create(c_name.as_ptr(), MFD_CLOEXEC | MFD_ALLOW_SEALING) };

    if fd < 0 {
        return Err(io::Error::last_os_error().to_string().into());
    }

    // 2. Write the payload bytes to the in-memory file
    let bytes_written =
        unsafe { libc::write(fd, payload.as_ptr() as *const libc::c_void, payload.len()) };

    if bytes_written != payload.len() as isize {
        // If write fails or is incomplete, clean up the file descriptor
        unsafe {
            libc::close(fd);
        }
        return Err("Failed to write full payload to memfd".into());
    }

    // Optional: Seal the file to prevent modification, common for security/integrity
    // Note: The MFD_ALLOW_SEALING flag must be set during creation for this to work.
    let seals = libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_WRITE;
    if unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, seals) } == -1 {
        // Log a warning but continue if sealing fails (e.g., due to permissions)
        debug!(
            "memfd_create_dlopen: Failed to apply seals. Error: {}",
            io::Error::last_os_error()
        );
    }

    // 3. Construct the virtual path to the in-memory file
    // This path is necessary for dlopen to work, as dlopen expects a filesystem path.
    let dl_path = format!("/proc/self/fd/{}", fd);

    // 4. Use dlopen (via libloading) on the virtual path
    Ok(unsafe { Library::new(&dl_path)? })
}
