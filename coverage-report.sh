#!/bin/zsh

# Before running this, you'll need to run `rustup component add llvm-tools-preview`

set -euo pipefail

cargo llvm-cov show-env --export-prefix
export RUSTFLAGS=" -C instrument-coverage --cfg coverage --cfg trybuild_no_target"
export LLVM_PROFILE_FILE="$PWD/target/self-limiters-%m.profraw"
export CARGO_INCREMENTAL="0"
export CARGO_LLVM_COV_TARGET_DIR="$PWD/target"

source <(cargo llvm-cov show-env --export-prefix)

export CARGO_TARGET_DIR=$CARGO_LLVM_COV_TARGET_DIR
export CARGO_INCREMENTAL=1
cargo llvm-cov clean --workspace
cargo test
source "$VIRTUAL_ENV"/bin/activate
maturin develop
coverage run -m pytest tests

cargo llvm-cov report --ignore-filename-regex "errors|_tests|lib"
