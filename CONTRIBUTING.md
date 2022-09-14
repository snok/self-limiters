# Contributing

Contributions are always welcome, and if you're unsure about something, please don't hesitate
to open an issue.

The rest of this file is dedicated document development and releases for things
that I'll otherwise forget.

## Package version

The package's version lives in `cargo.toml`, but is set
in the release workflow based on the release tag. Release tags
should therefore always conform to `v{0-9}.{0-9}.{0-9}`

## Debugging the Lua scripts

Assuming you have a redis server running at `:6389` you can debug
a lua script by calling `redis-cli -u redis://127.0.0.1:6389 --ldb --eval src/semaphore/rpushnx.lua x 1`.

Just type `help` in the debugger for options.

## Coverage

Since some of our tests are written in Rust, and some are written in Python,
we've modelled our codecov setup after [this](https://github.com/cjermain/rust-python-coverage)
project. The process consists of running both test suites with individual coverage tools, then
patching the coverage data together via codecov (so far it's been a bit flaky, not always
picking up the python codecov)
