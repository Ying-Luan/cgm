#!/usr/bin/env bash

set -euo pipefail

BINARY_NAME="cgm"
GITEE_REPOSITORY="ying_luan/cgm"
GITHUB_REPOSITORY="Ying-Luan/cgm"
INSTALL_DIR="/usr/local/bin"
TARGET="x86_64-unknown-linux-gnu"

mirror="${CGM_MIRROR:-github}"

die() {
    printf 'Error: %s\n' "$*" >&2
    exit 1
}

download() {
    local url="$1"
    local output="$2"

    curl \
        --fail \
        --location \
        --proto '=https' \
        --show-error \
        --silent \
        --tlsv1.2 \
        --output "$output" \
        "$url"
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || die "Required command not found: $1"
}

[[ $# -eq 0 ]] || die "This script does not accept arguments"

for command_name in curl grep install mkdir mktemp rm sed sha256sum sudo tar uname xz; do
    require_command "$command_name"
done

[[ "$(uname -s)" == "Linux" ]] || die "Only Linux is supported"
[[ "$(uname -m)" == "x86_64" ]] || die "Unsupported architecture: $(uname -m)"

if cgm status 2>/dev/null \
    | sed 's/\x1b\[[0-9;]*m//g' \
    | grep "Daemon: ACTIVE" >/dev/null; then
    die "CGM daemon is running. Run 'sudo cgm stop' before installing"
fi

archive_name="$BINARY_NAME-$TARGET.tar.xz"
checksum_name="$archive_name.sha256"

temp_dir="$(mktemp -d)"
trap 'rm -rf "$temp_dir"' EXIT

case "$mirror" in
    gitee)
        release_metadata="$(download "https://gitee.com/api/v5/repos/$GITEE_REPOSITORY/releases/latest" /dev/stdout)" \
            || die "Failed to query latest Gitee release"
        if [[ "$release_metadata" =~ \"tag_name\":\"([^\"]+)\" ]]; then
            release_tag="${BASH_REMATCH[1]}"
        else
            die "Failed to resolve latest Gitee release"
        fi
        release_url="https://gitee.com/$GITEE_REPOSITORY/releases/download/$release_tag"
        ;;
    github)
        release_url="https://github.com/$GITHUB_REPOSITORY/releases/latest/download"
        ;;
    *)
        die "Unsupported mirror: $mirror"
        ;;
esac

# Download the release archive and checksum
printf 'Downloading %s from %s...\n' "$archive_name" "$mirror"
download "$release_url/$archive_name" "$temp_dir/$archive_name" \
    || die "Failed to download release archive"
download "$release_url/$checksum_name" "$temp_dir/$checksum_name" \
    || die "Failed to download release checksum"

# Verify checksum
read -r expected_checksum _ <"$temp_dir/$checksum_name"
[[ "$expected_checksum" =~ ^[0-9a-fA-F]{64}$ ]] || die "Invalid checksum file"
actual_checksum_line="$(sha256sum "$temp_dir/$archive_name")"
actual_checksum="${actual_checksum_line%% *}"
[[ "$actual_checksum" == "$expected_checksum" ]] || die "Checksum verification failed"

# Extract the archive
mkdir "$temp_dir/extracted"
tar -xJf "$temp_dir/$archive_name" -C "$temp_dir/extracted"
archive_dir="${archive_name%.tar.xz}"
source_binary="$temp_dir/extracted/$archive_dir/$BINARY_NAME"
[[ -f "$source_binary" ]] || die "Release archive does not contain $BINARY_NAME"

# Install the binary
sudo mkdir -p "$INSTALL_DIR"
install_path="$INSTALL_DIR/$BINARY_NAME"
sudo install -m 0755 "$source_binary" "$install_path"

printf 'Installed %s to %s\n' "$("$install_path" --version)" "$install_path"
