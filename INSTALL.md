# Installing DMLPact

DMLPact publishes native archives for Linux x86_64, macOS Apple Silicon and
Intel, and Windows x86_64. The commands below download only the archive for the
current machine, verify its checksum and GitHub build provenance, and install
the binary under the current user account.

## macOS or Linux

The [GitHub CLI](https://cli.github.com/) is required. Run this from a clean
temporary directory:

```sh
version=v0.3.0
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) platform=macos-aarch64; checksum() { shasum -a 256 "$@"; } ;;
  Darwin-x86_64) platform=macos-x86_64; checksum() { shasum -a 256 "$@"; } ;;
  Linux-x86_64) platform=linux-x86_64; checksum() { sha256sum "$@"; } ;;
  *) printf 'Unsupported platform: %s-%s\n' "$(uname -s)" "$(uname -m)" >&2; exit 1 ;;
esac

archive="dmlpact-${version}-${platform}.tar.gz"
gh release download "$version" --repo yhay81/dmlpact \
  --pattern "$archive" --pattern SHA256SUMS
awk -v file="./$archive" \
  '$2 == file { print; found = 1 } END { if (!found) exit 1 }' \
  SHA256SUMS | checksum -c -
gh attestation verify "$archive" --repo yhay81/dmlpact
tar -xzf "$archive"
install -d "$HOME/.local/bin"
install -m 0755 "${archive%.tar.gz}/dmlpact" "$HOME/.local/bin/dmlpact"
"$HOME/.local/bin/dmlpact" --version
```

Add `$HOME/.local/bin` to `PATH` if it is not already present.

## Windows

Run PowerShell in a clean temporary directory:

```powershell
$ErrorActionPreference = "Stop"
$version = "v0.3.0"
$archive = "dmlpact-$version-windows-x86_64.zip"
gh release download $version --repo yhay81/dmlpact `
  --pattern $archive --pattern "SHA256SUMS"
if ($LASTEXITCODE -ne 0) { throw "Release download failed" }
$checksumLine = Get-Content SHA256SUMS |
  Where-Object { ($_ -split '\s+')[1] -eq "./$archive" }
if (-not $checksumLine) { throw "Archive checksum not found" }
$expected = ($checksumLine -split '\s+')[0].ToLowerInvariant()
$actual = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw "Checksum mismatch" }
gh attestation verify $archive --repo yhay81/dmlpact
if ($LASTEXITCODE -ne 0) { throw "Attestation verification failed" }
Expand-Archive $archive -DestinationPath .
$bin = Join-Path $HOME ".local\bin"
New-Item -ItemType Directory -Force $bin | Out-Null
Copy-Item "dmlpact-$version-windows-x86_64\dmlpact.exe" `
  (Join-Path $bin "dmlpact.exe") -Force
& (Join-Path $bin "dmlpact.exe") --version
if ($LASTEXITCODE -ne 0) { throw "Installed binary failed" }
```

Add `$HOME\.local\bin` to the user `PATH` if necessary.

## Build from source

Rust 1.85 or newer is required:

```sh
git clone https://github.com/yhay81/dmlpact.git
cd dmlpact
cargo install --path . --locked
dmlpact --version
```

## Update or remove

To update, repeat the verified installation with the desired immutable release
version. To remove a native installation, delete `$HOME/.local/bin/dmlpact` on
macOS/Linux or `$HOME\.local\bin\dmlpact.exe` on Windows. Plans, SQL files, and
receipts are not removed automatically.
