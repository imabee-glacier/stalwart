# Open-source conversion and validation

The conversion removes licensing as an executable capability decision. Features
are configured and authorized through the ordinary server settings and permissions.

## Removed mechanisms

- License parsing, signatures, hostname binding, validity dates, account entitlements,
  activation credentials, licensing HTTP requests, renewal retries and backoff.
- Startup and reload fallbacks that downgraded stores or disabled directories,
  telemetry, AI, templates, branding or retention when a license was unavailable.
- The license-renewal scheduler event, its scheduling and rescheduling paths,
  retry constants, startup logging and public licensing event/property definitions.
- Compile-time enterprise feature gates, Cargo feature definitions and corresponding
  Docker/CI build flags. There is one dashboard schema, not separate editions.
- Edition arguments and predicates in telemetry, no-op registry/SCIM entitlement
  contracts, the account API's edition marker, and the optional enterprise Core bundle.
- Dashboard edition state, read-only paid fields, locked navigation/actions,
  license forms, trial/purchase prompts and activation links.

AI, masked addresses, branding and alerts now live in their ordinary modules.
Core owns its retention, templates, AI and telemetry settings directly; they are
not attached through an optional licensed configuration.

## Stored data and client compatibility

The branding object keeps storage ID **40**, and its Get/Update permissions keep
IDs **363/364**. Its public names are now `Branding`, `sysBrandingGet` and
`sysBrandingUpdate`. New version-1 records encode only branding fields. The
version-0 reader consumes and discards the old credentials while preserving the
logo; rewriting an old record removes the credential bytes and slots. The
configuration migration script imports branding without importing activation keys.
Other IDs are not renumbered or reused.

Webadmin source is included in `webui/`; its provenance is recorded there. Its
reproducible ZIP is embedded in the server. Both fresh installations and records
with the exact old default download URL use that bundle, bypassing stale ZIP
caches and downloads. Custom application sources retain their configured behavior.
Custom API clients must adopt the renamed branding settings and the account
response without an edition field.

Legacy reduced-feature database layouts remain subject to storage-format
validation. Removing that check would risk decoding data with the wrong layout;
it is not a license check or an automatic data migration.

## Validation commands

Run from the repository root unless otherwise indicated:

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo check -p stalwart
cargo test --workspace --no-fail-fast
cargo test -p registry --test open_source_branding -p common --test runtime_settings
cargo test -p common --lib bundled_webadmin_tests
python3 -m pip install requests
python3 -m unittest discover -s resources/scripts/tests
npm ci --prefix webui
npm run typecheck --prefix webui
npm run lint --prefix webui
npm run format:check --prefix webui
npm test --prefix webui
npm run build --prefix webui
python3 webui/bundle.py --check
npm audit --prefix webui
cargo build -p stalwart --no-default-features --features sqlite
webui/node_modules/.bin/playwright install chromium
python3 resources/scripts/test_open_source_server.py --bootstrap
python3 resources/scripts/test_open_source_server.py --browser
git diff --check
```

The live smoke test starts and terminates its own disposable server and uses
synthetic credentials. It checks the served dashboard, account and schema APIs,
authorized tracing/metrics token issuance and rejection of unauthenticated access.
The browser test saves a logo through the real form and verifies it after reload;
its session uses HTTP Basic test credentials, not the unrelated OAuth login flow.
Browser requests to external origins are blocked.

The bundled-dashboard tests also cover upgrade from the old default URL and
preservation of custom application URLs. Runtime settings are tested through
registry writes and reloads. Migration tests verify that old activation secrets
are not exported.

The imported frontend's lockfile was updated for dependency advisories. A
`minimatch` override selects the patched 11.x release for the development tools;
the type check, linter, tests and production build validate that toolchain.

## Scope of the evidence

The repository's separate `stalwart-tests` fixture repository is not present.
Its external protocol/integration tests therefore cannot be counted as executed.
Workspace tests exercise the tests included here; compiling all backend features
does not test connections to production FoundationDB, SQL, cloud, or coordinator
services. The live server/browser tests use SQLite and bootstrap/recovery setup.
No production deployment or complete mail-delivery acceptance test is represented
by these checks. OAuth token expiry, certificate renewal, cluster leases, configured
quotas, AI rate limits and data-retention timers remain ordinary server behavior.

## Recorded results (2026-11-04)

All commands above passed on the final implementation. The workspace test run
completed 72 test/doc-test targets with six executed tests and zero failures.
The separate targeted runs passed all three branding tests, the runtime reload
test, and both bundled-dashboard tests. Both Python migration tests and all four
frontend tests passed. The rebuilt SQLite binary passed the bootstrap smoke and
the authenticated server/browser smoke, including saving and reloading branding.
The frontend audit reported zero vulnerabilities. Both format checks, frontend
lint/type checking, schema checksums, deterministic bundle packaging and Git's
whitespace check passed. The all-target compile emits four pre-existing warnings
from the SMTP test-only DNS mock paths.

A scan of the rebuilt executable also found none of the former licensing URLs,
renewal/validation task names or licensed-account-limit messages. This complements
the source audit; it does not expand the deployment/test coverage described above.

## Passkeys and release validation (2026-11-05)

Passkeys now use the normal credential registry and OAuth authorization/code/device
flows. The dashboard includes Account → Credentials → Passkeys with enrollment,
rename and verified removal; the hosted login page includes passkey sign-in.
[PASSKEYS.md](PASSKEYS.md) describes origin configuration, cluster state storage,
revocation, compatibility and the test fixture.

Passed locally:

- Workspace/all-target checks with all features and with SQLite/PostgreSQL/MySQL.
- Workspace tests: 73 test/doc-test targets, 11 executed tests, zero failures.
  A subsequent focused HTTP run passed all four origin/name/expiry tests after
  adding the explicit stored-state expiry case. Two registry tests cover legacy
  account data and passkey/epoch round trips and metadata disclosure.
- Dashboard type checking, ESLint, Prettier, eight Vitest tests, production build,
  deterministic bundle check, and npm audit (zero vulnerabilities).
- Both Python migration tests, login asset consistency, cargo fmt, actionlint,
  and whitespace validation. Release archive naming and validation dependencies
  were also exercised locally.
- The rebuilt SQLite binary: bootstrap smoke, Branding save/reload browser smoke,
  and CTAP2 browser passkey lifecycle test. That test covers enrollment, rename,
  OAuth consent/PKCE/userinfo, device-grant tokens, scoped management, password
  plus MFA step-up, signature/UV/origin failures, cross-account proof rejection,
  concurrent replay, single-use management authorization, removal and existing
  token revocation. Desktop/light and mobile/dark views were inspected.

Release jobs now depend on the same reusable validation workflow. Validation and
release builds check out the triggering SHA and use the checked-in Cargo lockfile.
Generated assets are checked against local sources; they are not downloaded from
upstream. The installer downloads this fork's releases. Cleanup never deletes a
tag and only deletes draft releases. At that initial checkpoint, no commit, push or release had been performed
for the passkey changes.

### Unresolved dependency audit

`cargo audit --json` fails with three package findings across two advisories:

- `lz4_flex` 0.10.0 through optional `zenoh` → `zenoh-transport` 1.10.2:
  [RUSTSEC-2026-0041](https://rustsec.org/advisories/RUSTSEC-2026-0041.html).
  The direct application LZ4 dependencies use 0.14.0. Zenoh's locked dependency
  range cannot be fixed by a compatible lockfile update; it needs an upstream
  dependency change or a separately reviewed vendored patch/backend change.
- `rsa` 0.9.10 (existing common/key-generation dependencies) and 0.10.0-rc.19
  (`webauthn-rs-core` → `crypto-glue`):
  [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html).
  RustSec lists no patched version. Inspection of the WebAuthn verifier shows
  RSA public-key verification, not private-key signing/decryption. That narrows
  applicability for passkeys; it does not make the repository audit clean.

At that initial checkpoint no audit exceptions had been added, and the findings
blocked shipping. The publication follow-up below records the subsequent
remediation and narrowly reviewed exception. Cross-platform
release runners, production Redis/cluster behavior, physical authenticators and
external mail/database integration suites were not run locally. The external
schema generator and `stalwart-tests` repository are absent from this checkout.
The results above are evidence of the tested behavior, not a security guarantee.


## Publication follow-up (2026-11-05)

The dependency findings above have now been addressed: Zenoh transport is
vendored with LZ4 0.14.0; its two compressed-batch round-trip tests pass. Legacy
Zenoh RSA authentication/multilink features are disabled, with tested fail-closed
configuration checks. Remaining RSA use is covered by the expiring, graph-checked
applicability exception described in SECURITY_DEPENDENCIES.md. cargo-audit now
passes with that explicit exception; five informational warnings remain visible.
This is not a claim that the upstream RSA crate is patched.

The subsequent all-features/all-target check and workspace tests passed. The
coordinator security test and Python policy tests passed. CI now installs the
native libcurl headers whose absence caused the earlier GitHub build failure,
reserves disk for native builds, and checks Docker's vendored dependency sources.
The five reviewed Dependabot Actions updates are incorporated without replacing
any server, dashboard, activation-removal, or user-deleted files. The latest
remote README and AGENTS.md deletions were preserved as the publication base.

## POP3 removal (2026-11-05)

POP3 is removed rather than disabled: the `pop3` crate, its session manager,
listener wiring, default `pop3s` listener, autodiscovery/autoconfig/PACC entries,
DNS SRV and TLSA records, OAuth scopes, default permissions, connection metrics,
Docker `EXPOSE` ports and AppArmor bind rules for 110/995 are gone.

The storage identifiers (listener protocol 4, service protocol 2, permissions
161–166) remain reserved as `Retired*` variants so existing databases still
decode. They cannot be parsed from API input or offered in the schema. Saving a
legacy POP3 listener fails validation, and an existing one is skipped at
startup with a build warning.

Telemetry is stripped as well: the `Pop3Event` class and the four POP3 metrics
are deleted from `trc`, so no POP3 event, metric, description, Prometheus or
OpenTelemetry name exists in the dashboard, APIs or exporters. Their IDs
(events 330-347, metrics 14, 19, 194, 195) are documented as permanently
reserved. Stored data that still references them degrades gracefully: event and
metric filters drop the unknown entries and keep the rest, registry listings skip
and log an undecodable object instead of discarding every object of that type,
and metric history queries skip such rows (get reports them as not found).
The docker run examples in the upgrade guides no longer publish ports 110/995.

Retired permission IDs are also dropped wherever stored role or account
permission lists become permission sets, and the recovery administrator is
built from the active permission list rather than every bit. The live test
caught the administrator previously listing `retiredPop3*` permissions;
`crates/common/tests/removed_permissions.rs` now covers this, including granting
legacy roles that still store those IDs.

Validation: `cargo check --workspace --all-targets`, `cargo fmt --check`,
`cargo test -p registry` (including `tests/removed_pop3.rs`), the all-features and
SQLite/PostgreSQL/MySQL checks, Zenoh tests, the full workspace
test suite, Python policy tests, cargo-audit and dashboard checks. The live
server smoke test now also asserts that the served schema and account
permissions contain no POP3 entries; `--bootstrap`, `--browser` and
`--passkeys` all pass against the rebuilt SQLite binary.
