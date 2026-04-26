# Distribution

Senda ships through GitHub Releases. The release workflow at
`.github/workflows/release.yml` runs on every `v*` tag (or manually via the
Actions tab) and produces:

| Platform | Artefacts                                           |
| -------- | --------------------------------------------------- |
| macOS    | `Senda_<version>_universal.dmg`, `.app` archive     |
| Windows  | `Senda_<version>_x64-setup.exe` (NSIS), `.msi`      |
| Linux    | `.deb`, `.rpm`, `.AppImage`                         |

## Tagging a release

```bash
git tag v0.1.0
git push --tags
gh run list --workflow=release.yml
gh release view v0.1.0
```

The workflow drafts the release; promote it to public once you've smoke-tested
each platform's installer.

## Code signing

Phase 6 ships unsigned binaries — acceptable for two or three internal testers.
Promote to signed builds before any wider distribution.

| Tier              | Cost              | Notes                                         |
| ----------------- | ----------------- | --------------------------------------------- |
| Personal / POC    | $0                | macOS Gatekeeper warning, Windows SmartScreen warning |
| Early adopters    | ~150 €/year (OV)  | Reduces SmartScreen warning over time         |
| Public product    | ~400 €/year (EV)  | No SmartScreen warning at any download volume |
| macOS + Apple Dev | $99/year          | Notarization removes Gatekeeper warning       |

## Auto-update

`tauri-plugin-updater` is wired into the binary. To enable real updates:

1. Generate a signing keypair once:

   ```bash
   pnpm tauri signer generate -w ~/.tauri/senda.key
   ```

2. Replace `pubkey` in `apps/desktop/src-tauri/tauri.conf.json` with the public
   key the command prints.

3. Add the private key (and optional password) to repository secrets:
   `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

4. Flip `createUpdaterArtifacts` to `true` in `tauri.conf.json`.

After that, every release publishes a `latest.json` manifest that the running
app polls. If a newer signed bundle is available the user gets prompted.

## Local builds

```bash
pnpm tauri build                # builds for the current platform
pnpm tauri build --target universal-apple-darwin
pnpm tauri build --target x86_64-pc-windows-msvc
```

The first run will download Tauri's bundling tools (DMG, WiX, AppImage). On
Linux the same dependencies the CI workflow installs are required locally.
