SHELL := /bin/bash

.ONESHELL:

.SHELLFLAGS := -eu -o pipefail -c
ifeq ($(origin .RECIPEPREFIX), undefined)
  $(error This Make does not support .RECIPEPREFIX. Please use GNU Make 4.0 or later)
endif
.RECIPEPREFIX = >

$(VERBOSE).SILENT:

test:
> source <(cargo llvm-cov show-env --export-prefix)
> export CARGO_TARGET_DIR=$CARGO_LLVM_COV_TARGET_DIR
> export CARGO_INCREMENTAL=1
> cargo llvm-cov clean --workspace
> cargo test
> maturin develop
> coverage run -m pytest tests && coverage xml
> cargo llvm-cov report --lcov --output-path coverage.lcov
.PHONY: test

fmt:
> pre-commit run --all-files
.PHONY: fmt

fix:
> git add .
> cargo clippy --fix --allow-staged
.PHONY: fix
