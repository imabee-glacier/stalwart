# Dependency security review — 2026-11-05

CI runs cargo-audit and the applicability guard in
`resources/scripts/check_crypto_policy.py`. New advisories still fail the audit;
the exception below is limited by dependency versions, parent edges, compiled
features and an expiry date of 2027-02-05. A dependency update that changes these
conditions requires another review. The exception does not claim the RSA library
is fixed.

## RUSTSEC-2026-0041 — LZ4 decoder

Zenoh 1.10.2 selects vulnerable lz4_flex 0.10.0. The checked-in transport override
uses patched 0.14.0, preserving the LZ4 wire format and transport compression.
`vendor/README.md` records its provenance and single manifest change. The global
lockfile contains no 0.10 decoder. Compression round-trip tests and workspace
all-features compilation validate compatibility.

## RUSTSEC-2023-0071 — RSA private operations

The RustCrypto RSA advisory has no patched release. Initial graph review found
an affected network decryption path in Zenoh's optional public-key challenge
exchange and multilink, in addition to the following unaffected usages. The
Zenoh RSA features are now excluded; coordinator setup explicitly rejects their
configuration rather than silently dropping requested authentication. Legacy
username/password configuration is also rejected because this build does not
provide that backend. Existing deployments using these modes must configure a
single link with authenticated TLS. TLS/QUIC/TCP/UDP transports and compression
remain supported. This is an intentional security-related compatibility change.

The remaining RSA dependency paths were reviewed:

- rsa 0.9.10 → common 1.0.3: local PEM/DER parsing and key conversion in
  `config/mod.rs` and public-key parsing in `network/dkim.rs`.
- rsa 0.9.10 → mail-auth 0.15.1: local DKIM key generation and encoding.
  The selected mail signing backend is AWS-LC, and the RustCrypto signing backend
  is not enabled.
- rsa 0.10.0-rc.19 → crypto-glue 0.2.2 → webauthn-rs-core and
  webauthn-attestation-ca 0.6.2: public-key and certificate verification.
  WebAuthn never receives or uses an authenticator's private key.

JWT private-key operations also use AWS-LC via jsonwebtoken 11.1.0. No reviewed
path uses RustCrypto RSA for network private-key signing or decryption. The
advisory is therefore recorded in `.cargo/audit.toml` as a reviewed applicability
exception. The graph guard checks the above conditions on all workspace features
before that exception can permit a release. Any introduction of private-key
operations using `rsa`, a new dependent, backend feature or reviewed version
requires removing or re-justifying the exception.

Sources: [RSA advisory](https://rustsec.org/advisories/RUSTSEC-2023-0071.html),
[LZ4 advisory](https://rustsec.org/advisories/RUSTSEC-2026-0041.html), the pinned
crate sources, and the compiled all-features dependency graph. Informational
cargo-audit warnings (such as unmaintained crates) remain visible in CI logs.
