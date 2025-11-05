# RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none" \
# rustup run nightly cargo build --release

# rustup run nightly cargo build \
#     --release \
#     --no-default-features \
#     -Zbuild-std="core,std,alloc,proc_macro,panic_abort" \
#     -Zbuild-std-features="panic_immediate_abort"


# RUSTFLAGS="-Z build-std" \
rustup run nightly cargo build --release
