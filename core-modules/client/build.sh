# cargo clean

OBFUSCATION_KEY=abc123abc \
cargo build --release

export BINARY=./target/release/libclient.so


declare -a headers=(
    ".gnu_debuglink" # - Debug information link
    ".comment" #- Compiler version info
    ".shstrtab" #- Section header string table (only needed by tools like readelf)
    ".note.gnu.bu" ".note.gnu.build-id" # - Build ID note
    ".eh_frame" ".eh_frame_hdr" # Exception handling info (can break C++ exceptions if removed)
    ".gnu.version"
    #".gnu.version_r"
    # Symbol versioning (may be needed for some shared libraries)
    #".gnu.hash" # Hash table for symbol lookup optimization
)

# TODO: Implement FAKE section header comments and information
# Shuffle order of headers??

for section in "${headers[@]}"
do
    strip --remove-section="$section" $BINARY
    echo "Removed section header $section"
done
