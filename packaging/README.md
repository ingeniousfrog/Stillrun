# Packaging Stillrun

Stillrun is a macOS CLI, so the `ingeniousfrog/homebrew-tap` entry should be a
Formula, not a Cask. Casks are available for app bundles and trusted binary
artifacts, but this tap already keeps GUI apps in `Casks/` and CLI tools in
`Formula/`.

## User Install

```bash
brew tap ingeniousfrog/tap
brew install stillrun
```

Users can also install without tapping first:

```bash
brew install ingeniousfrog/tap/stillrun
```

## Release Flow

1. Bump `Cargo.toml` and `Cargo.lock` to the new version.
2. Run the local verification gate:

   ```bash
   cargo fmt --all --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```

3. Push `main`, tag the release, and push the tag:

   ```bash
   git tag v0.1.0
   git push origin main
   git push origin v0.1.0
   ```

4. Wait for the `Release` workflow to publish these assets:

   ```text
   stillrun-aarch64-apple-darwin.tar.gz
   stillrun-aarch64-apple-darwin.tar.gz.sha256
   stillrun-x86_64-apple-darwin.tar.gz
   stillrun-x86_64-apple-darwin.tar.gz.sha256
   ```

5. Render the Homebrew formula from the published checksums:

   ```bash
   ./packaging/render-homebrew-formula.sh 0.1.0 <aarch64_sha256> <x86_64_sha256>
   ```

6. Copy `packaging/homebrew/stillrun.rb` to
   `ingeniousfrog/homebrew-tap/Formula/stillrun.rb`, then validate from the tap:

   ```bash
   brew audit --formula Formula/stillrun.rb
   brew install --formula Formula/stillrun.rb
   brew test stillrun
   ```

7. Commit and push the tap.
