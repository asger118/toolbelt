#!/bin/sh
# Installs (or updates) toolbelt's `tb` on Linux.
#   curl -fsSL https://raw.githubusercontent.com/asger118/toolbelt/main/install.sh | sh
set -eu
repo="asger118/toolbelt"
dir="$HOME/.local/bin"

case "$(uname -m)" in
  x86_64 | amd64) asset="tb-linux-x64.tar.gz" ;;
  aarch64 | arm64) asset="tb-linux-arm64.tar.gz" ;;
  *) echo "toolbelt: unsupported CPU $(uname -m)" >&2; exit 1 ;;
esac

mkdir -p "$dir"
curl -fsSL "https://github.com/$repo/releases/latest/download/$asset" | tar -xz -C "$dir" tb
chmod +x "$dir/tb"
echo "Installed $("$dir/tb" --version) to $dir"

case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "Add $dir to your PATH, e.g. in ~/.bashrc:  export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
esac
shell_name=$(basename "${SHELL:-bash}")
echo "Enable Ctrl+S by adding this line to your ~/.${shell_name}rc:"
echo "  eval \"\$(tb init $shell_name)\""
