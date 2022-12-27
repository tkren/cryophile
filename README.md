# Permafrust

![build](https://github.com/tkren/permafrust/actions/workflows/build.yml/badge.svg)
![sec](https://github.com/tkren/permafrust/actions/workflows/sec.yml/badge.svg)

This is the Permafrust backup daemon.

```shell
dd if=/dev/random count=4096 | PARMAFRUST_LOG_STYLE= PERMAFRUST_LOG='warn,permafrust=trace' RUST_BACKTRACE=full cargo run --  -b /tmp backup -v UUID -o DATE
```

If notify fails, we need to bump max_user_instances, see <https://stackoverflow.com/a/71082431/2982090>

```shell
sudo sysctl fs.inotify.max_user_instances=512
```

## Generate encryption key

```shell
sq key generate --cipher-suite cv25519 --can-encrypt storage --cannot-authenticate --cannot-sign --export permafrust-key.pgp
```

```shell
sq inspect permafrust-key.pgp
permafrust-key.pgp: Transferable Secret Key.

    Fingerprint: 06A457651A730003DCC7BB20E1A8CDDAF1BDCEB3
Public-key algo: EdDSA
Public-key size: 256 bits
     Secret key: Unencrypted
  Creation time: 2022-12-26 20:55:41 UTC
Expiration time: 2025-12-26 14:22:02 UTC (creation time + P1095DT62781S)
      Key flags: certification

         Subkey: 62CC3D4C18C65A707CEC689DB1E96BA621037299
Public-key algo: ECDH
Public-key size: 256 bits
     Secret key: Unencrypted
  Creation time: 2022-12-26 20:55:41 UTC
Expiration time: 2025-12-26 14:22:02 UTC (creation time + P1095DT62781S)
      Key flags: data-at-rest encryption
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
