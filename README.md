# Permafrust

![build](https://github.com/tkren/permafrust/actions/workflows/build.yml/badge.svg)
![sec](https://github.com/tkren/permafrust/actions/workflows/sec.yml/badge.svg)

This is the Permafrust backup daemon.

## Backup and Restore

```shell
dd if=/dev/random count=4096 \
   | PERMAFRUST_LOG_STYLE= PERMAFRUST_LOG='warn,permafrust=trace' RUST_BACKTRACE=full cargo run -- -S /tmp backup -v UUID -p PREFIX
```

If notify fails, we need to bump max_user_instances, see <https://stackoverflow.com/a/71082431/2982090>

```shell
sudo sysctl fs.inotify.max_user_instances=512
```

## Generate encryption key and certificate

```shell
sq key generate --cipher-suite cv25519 --can-encrypt storage --cannot-authenticate --cannot-sign --output permafrust-key.pgp
```

```shell
sq key extract-cert --output permafrust-cert.pgp permafrust-key.pgp
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

```shell
sq inspect permafrust-cert.pgp                                                                                                                                                  (main)permafrust
permafrust-cert.pgp: OpenPGP Certificate.

    Fingerprint: 06A457651A730003DCC7BB20E1A8CDDAF1BDCEB3
Public-key algo: EdDSA
Public-key size: 256 bits
  Creation time: 2022-12-26 20:55:41 UTC
Expiration time: 2025-12-26 14:22:02 UTC (creation time + P1095DT62781S)
      Key flags: certification

         Subkey: 62CC3D4C18C65A707CEC689DB1E96BA621037299
Public-key algo: ECDH
Public-key size: 256 bits
  Creation time: 2022-12-26 20:55:41 UTC
Expiration time: 2025-12-26 14:22:02 UTC (creation time + P1095DT62781S)
      Key flags: data-at-rest encryption

```

## Configuration

Default configuration will be read from
`~/.config/permafrust/permafrust.toml`, and if this file is not
available then `/etc/permafrust/permafrust.toml` will be tried
next. If both fail to exist, the standard configuration will be
empty. If you pass `--config path/to/permafrust.toml`, `permafrust`
will only read `path/to/permafrust.toml` and fail if the file does not
exist.


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

## License

Permafrust is dual-licensed under the Apache License, Version 2.0
[LICENSE-APACHE](LICENSE-APACHE) or
<http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
[LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>, at
your option.
