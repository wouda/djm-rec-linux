# Spec Delta

## Purpose

Automates cross-platform Linux binary compilation, tarball packaging, and GitHub Releases distribution on version tag events with direct download badges.

## ADDED Requirements

### Requirement: Cross-Architecture Release Builds
The GitHub Actions workflow SHALL automatically compile release binaries for both `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` upon push of a version tag matching `v*`.

#### Scenario: Tag push triggers matrix build
- **WHEN** a git tag matching `v*` is pushed to GitHub
- **THEN** the workflow compiles `djm-box`, `djm-daemon`, `djm-tui`, and `djm-remote` with release optimizations for both target architectures

### Requirement: Tarball Packaging and Release Publishing
The workflow SHALL package the compiled binaries, systemd unit templates, appliance setup scripts, and documentation into compressed tarballs (`.tar.gz`) with SHA-256 checksums, and publish them to a GitHub Release.

#### Scenario: Release assets published
- **WHEN** cross-compilation jobs complete successfully
- **THEN** release tarballs for x86_64 and aarch64 along with checksum files are attached to the newly created GitHub Release

### Requirement: Documentation Download Links
The repository README SHALL display a GitHub release badge and direct installation instructions linking to the latest pre-built release downloads.

#### Scenario: User visits project repository
- **WHEN** a user navigates to the project GitHub page
- **THEN** the README provides clickable links to download pre-compiled release bundles for x86_64 and Raspberry Pi architectures
