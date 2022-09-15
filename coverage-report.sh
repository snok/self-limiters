#!/bin/zsh

set -euo pipefail

source <(cargo llvm-cov show-env --export-prefix)

CARGO_TARGET_DIR=$CARGO_LLVM_COV_TARGET_DIR \
  CARGO_INCREMENTAL=1 \
  && cargo llvm-cov clean --workspace \
  && cargo test

source "$VIRTUAL_ENV"/bin/activate \
  && maturin develop \
  && coverage run -m pytest tests

cargo llvm-cov report --ignore-filename-regex _errors
