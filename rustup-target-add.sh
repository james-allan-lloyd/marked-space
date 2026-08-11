set -xe
declare -A ARCH_TO_TOOLCHAIN=(
  ["arm64"]="aarch64-unknown-linux-gnu"
  ["amd64"]="x86_64-unknown-linux-gnu"
)
# rustc --print=target-list
CARGO_TARGET=${ARCH_TO_TOOLCHAIN[${TARGETARCH}]}
if [[ -z "$CARGO_TARGET" ]]; then
  echo "Unknown TARGETARCH \"${TARGETARCH}\"" 1>&2
  exit 1
fi

rustup target add $CARGO_TARGET

# use install here so we don't have to map in the run images.
# --locked builds the versions in Cargo.lock: cargo install otherwise re-resolves
# dependencies, and picks up releases that need a newer rustc than the image has.
cargo install --locked --target $CARGO_TARGET --path .
