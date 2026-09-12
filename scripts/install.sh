#!/usr/bin/env bash
set -e

REPO="${VIDOWN_REPO:-TheNobodyBaruah/Vidown}"
INSTALL_DIR="${VIDOWN_INSTALL_DIR:-/usr/local/bin}"
BIN_NAME="vidown"
LINK_NAME="Vidown"

# Check for root/sudo privileges
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    if command -v sudo >/dev/null 2>&1; then
        SUDO="sudo"
    else
        echo "Error: Root privileges are required. Please run this script as root or install sudo." >&2
        exit 1
    fi
fi

# Verify sudo privileges early, handling piped stdin (e.g. curl ... | bash)
if [ -n "$SUDO" ]; then
    if [ ! -t 0 ] && (exec </dev/tty) 2>/dev/null; then
        $SUDO -v </dev/tty
    else
        $SUDO -v
    fi
fi

# Handle uninstallation
case "$1" in
    --uninstall|-u|uninstall|-Uninstall)
        echo "==> Uninstalling Vidown..."
        removed=0
        if [ -f "$INSTALL_DIR/$BIN_NAME" ] || [ -L "$INSTALL_DIR/$BIN_NAME" ]; then
            $SUDO rm -f "$INSTALL_DIR/$BIN_NAME"
            echo "Removed $INSTALL_DIR/$BIN_NAME"
            removed=1
        fi
        if [ -f "$INSTALL_DIR/$LINK_NAME" ] || [ -L "$INSTALL_DIR/$LINK_NAME" ]; then
            $SUDO rm -f "$INSTALL_DIR/$LINK_NAME"
            echo "Removed $INSTALL_DIR/$LINK_NAME"
            removed=1
        fi
        if [ $removed -eq 1 ]; then
            echo "==> Vidown uninstalled successfully."
        else
            echo "==> Vidown is not installed in $INSTALL_DIR."
        fi
        exit 0
        ;;
esac

echo "==> Installing Vidown..."

# Helper functions for downloading
fetch() {
    local url="$1"
    if command -v curl >/dev/null 2>&1; then
        curl -sSL -H "Accept: application/vnd.github.v3+json" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "$url"
    else
        echo "Error: Neither curl nor wget was found. Please install curl or wget." >&2
        exit 1
    fi
}

download_file() {
    local url="$1"
    local dest="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$dest"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$dest"
    else
        echo "Error: Neither curl nor wget was found." >&2
        exit 1
    fi
}

# Fetch latest release metadata from GitHub API
API_URL="${VIDOWN_API_URL:-https://api.github.com/repos/${REPO}/releases/latest}"
echo "==> Checking for latest release from ${REPO}..."
RELEASE_JSON=$(fetch "$API_URL" || true)

# Extract asset download URL for linux-x86_64
DOWNLOAD_URL=""
if command -v jq >/dev/null 2>&1; then
    DOWNLOAD_URL=$(echo "$RELEASE_JSON" | jq -r '.assets[]? | select(.name | endswith("linux-x86_64.tar.gz")) | .browser_download_url' 2>/dev/null | head -n 1 || true)
fi

if [ -z "$DOWNLOAD_URL" ] || [ "$DOWNLOAD_URL" = "null" ]; then
    DOWNLOAD_URL=$(echo "$RELEASE_JSON" | grep -Eo '(https?|file)://[^"[:space:]]*linux-x86_64\.tar\.gz' | head -n 1 || true)
fi

if [ -z "$DOWNLOAD_URL" ] || [ "$DOWNLOAD_URL" = "null" ]; then
    echo "==> Falling back to direct latest release download URL..."
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/vidown-linux-x86_64.tar.gz"
fi

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

ARCHIVE_PATH="$TMP_DIR/vidown-linux-x86_64.tar.gz"
echo "==> Downloading $DOWNLOAD_URL..."
download_file "$DOWNLOAD_URL" "$ARCHIVE_PATH"

echo "==> Extracting release archive..."
tar -xzf "$ARCHIVE_PATH" -C "$TMP_DIR"

BIN_SRC=$(find "$TMP_DIR" -type f -name "$BIN_NAME" | head -n 1)
if [ -z "$BIN_SRC" ]; then
    echo "Error: Binary '$BIN_NAME' not found in downloaded archive." >&2
    exit 1
fi

echo "==> Installing binary to $INSTALL_DIR/$BIN_NAME..."
$SUDO mkdir -p "$INSTALL_DIR"
$SUDO cp "$BIN_SRC" "$INSTALL_DIR/$BIN_NAME"
$SUDO chmod 755 "$INSTALL_DIR/$BIN_NAME"

echo "==> Creating symlink $INSTALL_DIR/$LINK_NAME -> $INSTALL_DIR/$BIN_NAME..."
$SUDO rm -f "$INSTALL_DIR/$LINK_NAME"
$SUDO ln -sfn "$INSTALL_DIR/$BIN_NAME" "$INSTALL_DIR/$LINK_NAME"

# Verify installed binary and display version
VER_OUTPUT=$("$INSTALL_DIR/$BIN_NAME" --version 2>/dev/null || true)
if [ -n "$VER_OUTPUT" ]; then
    echo "==> Installed binary version: $VER_OUTPUT"
fi

echo "==> Vidown installed successfully!"
echo "==> You can now run 'Vidown' or 'vidown' in your terminal."

# Check if INSTALL_DIR is in PATH
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        echo "Warning: $INSTALL_DIR is not currently in your PATH environment variable."
        echo "Add 'export PATH=\"$INSTALL_DIR:\$PATH\"' to your shell profile (~/.bashrc, ~/.zshrc, etc.)."
        ;;
esac
