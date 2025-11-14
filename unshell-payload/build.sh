OBFUSCATION_KEY=abc123abc \
RUST_LOG=info \
cargo run --no-default-features $@ --release # $(ls ../*/target/release/*.so)
