# Dependency patches

`zenoh-transport` is the unmodified crates.io 1.10.2 source, except that its
normalized Cargo.toml selects `lz4_flex` 0.14.0 instead of 0.10.0. The original
manifest is retained; the upstream revision and archive checksum are recorded below. The LZ4 block
wire format and the five compression/decompression API calls are unchanged.

This replaces the decoder affected by RUSTSEC-2026-0041. The patch preserves
Zenoh transport compression and is checked with the workspace all-features
build and the transport's compression round-trip tests. Remove this override
when a supported upstream release selects a patched decoder.

Upstream git revision: `cc839802639e3b7ebea50990f7a819fe8990b0fe`. Source package checksum:
`b652b7a1a79d23b601920b425747d60c4838f44d64151bf77a963f7508de739b`.

The standalone test Cargo.lock is checked in so CI runs the vendored transport
round-trip tests with `--locked`; it is separate from the server dependency lock.
