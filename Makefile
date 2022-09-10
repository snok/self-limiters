SHELL := /bin/bash

.ONESHELL:

.SHELLFLAGS := -eu -o pipefail -c
ifeq ($(origin .RECIPEPREFIX), undefined)
  $(error This Make does not support .RECIPEPREFIX. Please use GNU Make 4.0 or later)
endif
.RECIPEPREFIX = >

$(VERBOSE).SILENT:

test:
> cargo llvm-cov clean --workspace
> cargo test
> maturin develop
> coverage run -m pytest tests && coverage xml
> cargo llvm-cov report
.PHONY: test

fmt:
> pre-commit run --all-files
.PHONY: fmt

fix:
> git add .
> cargo clippy --fix --allow-staged
.PHONY: fix
