# OBFUSCATION_KEY=abc123abc \
# RUST_LOG=info \
# cargo run --no-default-features $@ --release # $(ls ../*/target/release/*.so)

OBFUSCATION_KEY=abc123abc \

cargo build $@ --profile release

# RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none" \
# cargo +nightly build \
#   -Z build-std=std,panic_abort \
#   -Z build-std-features="optimize_for_size" \
#   $@ \
#   --profile release
