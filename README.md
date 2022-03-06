![build](https://github.com/tkren/permafrust/actions/workflows/build.yml/badge.svg)
![sec](https://github.com/tkren/permafrust/actions/workflows/sec.yml/badge.svg)

# Permafrust

This is the Permafrust backup daemon.

```shell
dd if=/dev/random count=4096 | PARMAFRUST_LOG_STYLE= PERMAFRUST_LOG=trace cargo run --  -b /tmp backup -v UUID -o DATE
```

## Update dependencies

```shell
cargo update -v
```

## Format code

```shell
cargo fmt -v --all
```

## Static checks

```shell
cargo check
cargo clippy
```

## Build

```shell
cargo build
```

## Test

```shell
RUST_BACKTRACE=full cargo test -- --nocapture
```

## Benchmark

```shell
cargo bench
```
