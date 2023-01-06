# Changelog

All notable changes to this project will be documented in this file.

## [0.2.2] - 2023-01-04

### Bug Fixes

- Fixed a memory leak where mmproxy doesn't free pipe fds after failing to set its buffer size.

## [0.2.1] - 2023-01-04

### Features

- Introduced context messages for each error message. See [#13](https://github.com/saiko-tech/mmproxy-rs/pull/13).

## [0.1.1] - 2022-12-30

### Features

- Made the TCP proxy to be zero-copy whenever possible. See [#10](https://github.com/saiko-tech/mmproxy-rs/pull/10).

### Documentation

- MIT license added.
- TCP benchmarks added.

### Miscellaneous Tasks

- Published the project on [crates.io](https://crates.io/crates/mmproxy).

## [0.1.0] - 2022-12-15

- Initial version of the project.
