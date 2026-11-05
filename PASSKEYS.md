# Passkeys

Passkeys are available under **Account → Credentials → Passkeys**, and on the hosted OAuth sign-in and device authorization pages. They use the account's normal permissions, tenant rules, consent, scopes, PKCE, token issuance, IP restrictions, and session revocation. Password and app-password sign-in remain available. Recovery administrators and accounts managed by an external directory cannot enroll local passkeys.

## Deployment

Use the configured public HTTPS server URL, including the correct port. Reverse proxies must preserve that public authority. The relying party ID is that URL's hostname; subdomains and alternate ports are not automatically trusted. POST requests require an exact matching Origin header. HTTP is supported only for `localhost` requests from a loopback address during development.

All nodes serving one origin must share the configured lookup store (for example Redis). Ceremonies live there for five minutes, with an additional explicit expiry check. Random 256-bit opaque handles reach the browser; serialized verification state never does. Each handle can be consumed once, including when concurrent requests race. Management authorizations expire after one minute and are also single-use. A deployment that uses separate node-local lookup stores must keep ceremonies on the same node.

Changing a hostname does not migrate existing credentials to the new relying party. The account page identifies keys registered at another address. Enroll at the new address using a password or another existing sign-in method.

## Security design

The pinned `webauthn-rs` 0.6.2 relying-party library performs challenge, origin, relying-party hash, signature, user-presence, and mandatory user-verification checks. Its high-level Passkey API supports device-bound and synced passkeys. Registration requests discoverable credentials; authenticator transport is not restricted. Private keys remain with the authenticator or password manager. The `danger-allow-state-serialisation` feature is used only to persist library ceremony state in the server-side lookup store, never to accept client-supplied ceremony state.

Enrollment and removal require a fresh password verification (including the account's configured OTP), or a freshly verified existing passkey. Bearer sessions alone cannot enroll a key. Secondary credentials cannot manage passkeys. Renaming is a metadata-only authenticated action. Accounts may enroll up to twenty keys. Generic registry JSON patches cannot create or replace verified key material. Credential IDs are globally unique per relying party through the registry's atomic unique index.

Verification rereads current account data and checks its security stamp before accepting a proof. Signature counters, backup state, and last-used dates are persisted with optimistic concurrency control. Key removal changes a server-managed credential epoch, invalidates cached access tokens, and revokes OAuth grants. Adding a key and renaming it preserve existing sessions. Legacy accounts default to epoch zero and retain their original token version until a revocation occurs. Removing the last key through the account API requires another password sign-in method.

The registry's public JMAP representation exposes passkey names, IDs, and usage dates, never serialized verification keys or user handles. Store backups retain the complete record; public JSON representations are not credential backup formats.

## Validation

Build the SQLite server, install dashboard dependencies and Chromium, then run:

```sh
cargo build -p stalwart --no-default-features --features sqlite
npm ci --prefix webui
webui/node_modules/.bin/playwright install chromium
python3 resources/scripts/test_open_source_server.py --passkeys
```

This starts a disposable database, creates native users and an OAuth client, and uses Chromium's CTAP2 virtual authenticator against the actual dashboard and hosted login pages. Unit tests cover legacy storage, credential round trips, public metadata, strict origin policy, names, and browser binary serialization. The external upstream `stalwart-tests` repository is not included in this checkout.

The schema Rust files and compressed JSON are maintained together here. Their original external generator is absent from this repository. Any future regeneration must retain the Passkey variant (tag 3), the appended account epoch (field 15), the global per-RP uniqueness index, and the public metadata-only representation.
