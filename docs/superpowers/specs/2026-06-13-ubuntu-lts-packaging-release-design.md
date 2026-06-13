# Ubuntu LTS Packaging and Release Design

## Goal

Publish installable Debian packages for `rust-imager` on Ubuntu LTS releases
from 18.04 through 26.04, for both `amd64` and `arm64`. A release must prove
that the same architecture-specific package installs and starts on every
supported LTS before GitHub Release assets are published.

## Supported Matrix

The release matrix is:

| Architecture | Ubuntu releases |
| --- | --- |
| `amd64` | 18.04, 20.04, 22.04, 24.04, 26.04 |
| `arm64` | 18.04, 20.04, 22.04, 24.04, 26.04 |

Support is fail-closed. If an official Ubuntu container image is unavailable,
cannot install the package, or cannot run the smoke tests, the release job
fails. The workflow must not silently omit an advertised LTS version.

## Build Compatibility Strategy

One package is produced per architecture, not per Ubuntu release. Release
binaries are built in an Ubuntu 18.04 userspace so their dynamically linked
glibc requirements do not exceed the oldest supported host.

The Rust toolchain remains pinned by `rust-toolchain.toml`. Native dependencies
are compiled in the 18.04 build environment. The workflow checks the resulting
binary for unresolved shared libraries before packaging it.

This produces two package assets:

- `rust-imager_<version>_amd64.deb`
- `rust-imager_<version>_arm64.deb`

LTS-specific package names are intentionally avoided because they imply
different binaries where no distribution-specific behavior is required.

## Debian Package Layout

The package installs:

- `/usr/sbin/rust-imager`
- `/usr/share/doc/rust-imager/README.md`
- `/usr/share/doc/rust-imager/CHANGELOG.md`
- `/usr/share/doc/rust-imager/copyright`

The package metadata uses:

- package: `rust-imager`
- section: `utils`
- priority: `optional`
- architecture: mapped from Rust target to Debian `amd64` or `arm64`
- version: the workspace version, with an optional non-release CI suffix
- maintainer: `donghoonpark <donghun94@gmail.com>`
- homepage: `https://github.com/donghoonpark/rust-imager`
- dependencies: `e2fsprogs, fdisk, mount, parted, util-linux`

The runtime dependencies cover `e2fsck`, `resize2fs`, `sfdisk`, `mount`,
`umount`, `partprobe`, `lsblk`, `findmnt`, `blkid`, `swapon`, `setsid`, and the
utilities used by the installed first-boot service.

Package assembly uses `dpkg-deb` and a repository script rather than a
Cargo-specific packaging plugin. This keeps package contents and metadata
explicit and makes local reproduction straightforward.

## Repository Components

### Package Builder

`packaging/debian/build-deb.sh` accepts the compiled binary, package version,
Debian architecture, and output directory. It validates all inputs, creates a
temporary package root, writes `DEBIAN/control`, installs documentation and the
binary with deterministic permissions, then invokes `dpkg-deb --build`.

The script must reject malformed Debian versions, unsupported architectures,
missing binaries, and version mismatches that could produce misleading assets.

### Package Smoke Test

`packaging/debian/test-package.sh` installs a supplied package and verifies:

1. package metadata reports the expected version and architecture;
2. `/usr/sbin/rust-imager` is executable;
3. `rust-imager --version` reports the workspace version;
4. `rust-imager --help` exits successfully;
5. `ldd` reports no unresolved libraries;
6. every required external command is available after dependency installation.

The smoke test does not perform destructive imaging. Existing privileged
integration and QEMU workflows remain responsible for device mutation and
first-boot behavior.

## GitHub Actions Workflow

`.github/workflows/release.yml` runs on:

- pushes of tags matching `v*`;
- manual `workflow_dispatch`.

The workflow has four phases:

1. **Validate metadata:** require a clean semantic version tag for tag builds
   and require it to equal the Cargo workspace version.
2. **Build packages:** build the release binary in Ubuntu 18.04 for `amd64` and
   `arm64`, assemble each `.deb`, and upload it as an Actions artifact.
3. **Compatibility matrix:** install and smoke-test each architecture package
   in official Ubuntu 18.04, 20.04, 22.04, 24.04, and 26.04 containers.
4. **Publish:** after every matrix job passes, generate `SHA256SUMS` and create
   or update the tag's GitHub Release with the two packages and checksum file.

The workflow runs on an `ubuntu-24.04` `amd64` GitHub-hosted runner. It executes
the `amd64` build natively inside an Ubuntu 18.04 container and executes the
`arm64` build inside an Ubuntu 18.04 container through Docker Buildx and QEMU
user-mode emulation. Compatibility tests use the same native-or-emulated
execution rule. Tests must execute binaries under their matching architecture
rather than only inspecting package metadata.

Manual runs execute validation, build, and compatibility testing and retain the
packages as Actions artifacts. They never create a GitHub Release.

The publish job receives `contents: write`; all preceding jobs use read-only
repository permissions. GitHub Actions dependencies are pinned to immutable
commit SHAs.

## Versioning and Release Behavior

Official releases use tags such as `v0.1.0`. The tag version must exactly match
`workspace.package.version` after removing the leading `v`.

Manual builds use a valid Debian prerelease version derived from the workspace
version and GitHub run number. They are test artifacts only and are never
attached to an official release.

The release body may be generated from GitHub's release notes. `CHANGELOG.md`
remains the human-maintained project history and must have a dated entry before
an official release tag is pushed.

Publishing is idempotent for a tag: rerunning a successful tag workflow replaces
assets with the same names only after the full compatibility matrix succeeds.

## Local Verification

Developers can reproduce package assembly on an Ubuntu or Debian host:

```bash
cargo build --workspace --release
packaging/debian/build-deb.sh \
  target/release/rust-imager \
  0.1.0 \
  amd64 \
  dist
```

The repository will provide a local matrix driver that uses Docker and QEMU
where needed to install-test both architectures across the supported LTS
images. It uses the same package scripts and container commands as CI to avoid
a separate untested release path.

## Documentation

`README.md` documents:

- installation with `sudo apt install ./rust-imager_<version>_<arch>.deb`;
- the two supported architectures;
- the Ubuntu 18.04 through 26.04 compatibility promise;
- runtime tool dependencies installed by the package;
- GitHub Release downloads and checksum verification;
- the existing requirement to run destructive operations with `sudo`.

`docs/testing.md` documents local package and compatibility checks. The
changelog records the packaging and automated release pipeline.

## Acceptance Criteria

- A local package build produces a valid `.deb` with the documented contents,
  metadata, permissions, and dependencies.
- The packaged binary version equals the Cargo workspace version.
- Both `amd64` and `arm64` packages are built from an Ubuntu 18.04 userspace.
- The exact package for each architecture installs and passes smoke tests on
  Ubuntu 18.04, 20.04, 22.04, 24.04, and 26.04.
- Manual workflow runs upload test artifacts without creating releases.
- A matching `v*` tag publishes both packages and `SHA256SUMS` only after all
  ten compatibility jobs pass.
- A missing LTS image, architecture emulation failure, unresolved library,
  dependency failure, or version mismatch prevents publication.
