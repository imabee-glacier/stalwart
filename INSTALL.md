# Installing Stalwart

These instructions install this project's builds, published at
[github.com/imabee-glacier/stalwart/releases](https://github.com/imabee-glacier/stalwart/releases).
The upstream documentation at stalw.art covers configuration and administration,
but its install commands download the upstream build. Use the commands below
instead.

## Linux, macOS and FreeBSD

```sh
curl --proto '=https' --tlsv1.2 -sSf https://get.imabee.com/install.sh -o install.sh
sudo sh install.sh
```

`get.imabee.com` serves `install.sh` from this repository's `main` branch. Each
release also publishes the installer as a release asset, alongside its
checksum and signatures.

The script:

1. Creates a `stalwart` service account (`_stalwart` on macOS).
2. Downloads the archive for your platform from the latest release and
   verifies it against the release's published SHA-256 checksum. It refuses to
   install an archive that does not match.
3. Installs the binary and writes an environment file for the service.
4. Installs and starts a service: systemd, or an init.d script where systemd is
   absent (Linux), launchd (macOS) or rc.d (FreeBSD).

It needs root, `curl` or `wget`, `tar`, and `sha256sum`, `shasum` or `openssl`.

### Verifying the installer

Each release publishes `install.sh` with a checksum, a Sigstore bundle and a
GitHub build attestation. To install from a verified copy, download the
release's installer instead and check it before running it:

```sh
curl --proto '=https' --tlsv1.2 -sSfLO https://github.com/imabee-glacier/stalwart/releases/latest/download/install.sh
curl --proto '=https' --tlsv1.2 -sSfLO https://github.com/imabee-glacier/stalwart/releases/latest/download/install.sh.sha256
sha256sum -c install.sh.sha256      # macOS: shasum -a 256 -c install.sh.sha256
gh attestation verify install.sh --repo imabee-glacier/stalwart
```

The checksum detects a damaged download; the attestation, which needs the
[GitHub CLI](https://cli.github.com), proves the file was built by this
repository's release workflow. (`-L` is needed here because GitHub redirects
release downloads to its storage host.)

### Finishing setup

A fresh installation starts in bootstrap mode, with a temporary administrator
password printed to the service log. The script prints the command to find it,
for example:

```sh
journalctl -u stalwart -n 200 | grep -A8 'bootstrap mode'     # systemd
grep -A8 'bootstrap mode' /var/log/stalwart/stalwart.log      # init.d and FreeBSD
```

Sign in at `http://<your-server>:8080/admin` as `admin` with that password and
complete the setup. To choose the password instead, set
`STALWART_RECOVERY_ADMIN=admin:<password>` in the environment file and restart
the service. Remove it once setup is complete: the server warns at startup while
it remains set.

### Options

```text
sudo sh install.sh [--fdb] [PREFIX]
```

| Option   | Effect                                                         |
| -------- | -------------------------------------------------------------- |
| `--fdb`  | Install the FoundationDB build (Linux x86-64 and AArch64).     |
| `PREFIX` | Install everything under one directory instead of FHS paths.   |

Default locations:

| Item        | Linux and macOS               | FreeBSD                              | With `PREFIX`              |
| ----------- | ----------------------------- | ------------------------------------ | -------------------------- |
| Binary      | `/usr/local/bin/stalwart`     | `/usr/local/bin/stalwart`            | `PREFIX/bin/stalwart`      |
| Settings    | `/etc/stalwart/config.json`   | `/usr/local/etc/stalwart/config.json`| `PREFIX/etc/config.json`   |
| Environment | `/etc/stalwart/stalwart.env`  | `/usr/local/etc/stalwart/stalwart.env`| `PREFIX/etc/stalwart.env` |
| Logs        | `/var/log/stalwart/`          | `/var/log/stalwart/`                 | `PREFIX/logs/`             |
| Data        | `/var/lib/stalwart/`          | `/var/db/stalwart/`                  | `PREFIX/data/`             |

The environment file documents the variables the service reads, such as
`STALWART_HOSTNAME`, `STALWART_PUBLIC_URL` and `STALWART_RECOVERY_MODE`.

### Upgrading

Run the same two commands again. The script replaces the binary, keeps the
settings, environment file and data, and restarts the service. Read the
[changelog](CHANGELOG.md) first: upgrade notes at the top of each release
describe changes that need action.

### Supported platforms

| Platform | Architectures                                         |
| -------- | ----------------------------------------------------- |
| Linux    | x86-64, AArch64, ARMv7, ARMv6 (glibc and musl builds) |
| macOS    | Apple silicon, Intel                                  |
| FreeBSD  | x86-64                                                |

On Linux the script picks the musl build when the system C library is musl
(for example Alpine), and the glibc build otherwise.

## Windows

Download `stalwart-x86_64-pc-windows-msvc.zip` and its `.sha256` file from the
[latest release](https://github.com/imabee-glacier/stalwart/releases/latest),
check the checksum (`Get-FileHash stalwart-x86_64-pc-windows-msvc.zip`), and
run `stalwart.exe --config <path>\config.json`.

## Docker

Images for x86-64, AArch64, ARMv7 and ARMv6 are published to the GitHub Container
Registry:

```sh
docker run -d --name stalwart \
  -p 443:443 -p 25:25 -p 587:587 -p 465:465 -p 143:143 -p 993:993 -p 4190:4190 -p 8080:8080 \
  -v stalwart-etc:/etc/stalwart -v stalwart-data:/var/lib/stalwart \
  ghcr.io/imabee-glacier/stalwart:latest
docker logs stalwart 2>&1 | grep -A8 'bootstrap mode'
```

Tags: `latest`, `v1`, `v1.0`, and each release such as `v1.0.4`. Append
`-alpine` (for example `latest-alpine`) for the musl-based Alpine image. Images
are signed with Cosign and carry a build attestation:

```sh
gh attestation verify oci://ghcr.io/imabee-glacier/stalwart:latest --repo imabee-glacier/stalwart
```

## Uninstalling (Linux with systemd)

```sh
sudo systemctl disable --now stalwart
sudo rm /etc/systemd/system/stalwart.service /usr/local/bin/stalwart
sudo systemctl daemon-reload
```

This keeps your settings, logs and mail data. To delete them as well, remove
`/etc/stalwart`, `/var/log/stalwart` and `/var/lib/stalwart`, then the
`stalwart` account (`sudo userdel stalwart`).
