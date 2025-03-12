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

# use install here so we don't have to map in the run images
cargo install --target $CARGO_TARGET --path .
