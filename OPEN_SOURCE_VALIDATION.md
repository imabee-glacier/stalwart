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
