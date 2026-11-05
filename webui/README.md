# Bundled Stalwart Webadmin

This dashboard is built and versioned with the server. It uses permissions and
server schemas to determine available operations; it has no edition or license checks.

Run `npm ci`, `npm run typecheck`, `npm run lint`, `npm test`, and `./build.sh`
in this directory to validate changes and regenerate `../resources/webui.zip`.
The ZIP is embedded in the server so installation and startup need no dashboard download.

Source imported from https://github.com/stalwartlabs/webui at revision
db31e845ae74341618113662b3e51ecfcb9d5961.

License: AGPL-3.0-only; third-party dependencies retain their own licenses.
