//! # Payload Modules
//!
//! Each file in this directory implements one payload capability.
//!
//! ## Adding a new module
//!
//! 1. Create a new file `modules/mymodule.rs`.
//! 2. Define a struct implementing [`unshell::tree::Endpoint`].
//! 3. Add `pub mod mymodule;` here.
//! 4. Register it in `main.rs`'s `build_tree()` function:
//!    `tree.register("/mymodule", modules::mymodule::MyModule);`
//!
//! ## Module path convention
//!
//! Modules are registered at relative paths (e.g., `/info`, `/shell`).
//! The full path on the network is `{base_path}/{relative_path}`, e.g.,
//! `/agents/abc123/info`.

pub mod info;
