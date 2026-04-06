# macOS Manual Upgrade Guide

This project keeps auto-update disabled for MVP. Upgrade is manual.

## 1. Build or download the installer

- Build locally:
  - `bash /Users/hanelalo/develop/dragon-li/scripts/build-macos-package.sh aarch64`
- Or download a released `.dmg` package.

## 2. Validate the package

- Run:
  - `bash /Users/hanelalo/develop/dragon-li/scripts/verify-macos-package.sh`
- This checks:
  - `.app` bundle exists with `Info.plist` and executable
  - `.dmg` exists and can be mounted
  - `.app` is present inside the mounted `.dmg`

## 3. Install / upgrade

1. Open the `.dmg`.
2. Drag `Dragon Li.app` into `/Applications`.
3. If prompted, choose **Replace**.

## 4. First launch after upgrade

1. Open `Dragon Li.app`.
2. Verify existing runtime data is still present under:
   - `~/.dragon-li/`
3. If macOS blocks launch (Gatekeeper), run once:
   - `xattr -dr com.apple.quarantine /Applications/Dragon\ Li.app`

## 5. Rollback (manual)

1. Keep the previous `.dmg` from old release.
2. Reinstall old `Dragon Li.app` by replacing in `/Applications`.
3. Runtime data remains in `~/.dragon-li/`.

## 6. Release notes checklist

Before publishing a new version, include:

- Version number and build date.
- New features / fixes.
- Known issues.
- Rollback note and previous stable version link.
