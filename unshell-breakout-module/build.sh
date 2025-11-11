OBFUSCATION_KEY=abc123abc \
RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none" \
cargo +nightly build \
  -Z build-std=std,panic_abort \
  -Z build-std-features="optimize_for_size" \
  --profile release $@

# upx ./target/release/libunshell_module_test.so
