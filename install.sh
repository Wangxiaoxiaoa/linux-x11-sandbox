#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$PROJECT_DIR/target/release/linux-x11-sandbox"
SKILL_SRC="$PROJECT_DIR/skills/linux-x11-sandbox"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { echo -e "${GREEN}[install]${NC} $*"; }
warn() { echo -e "${YELLOW}[warn]${NC} $*"; }
info() { echo -e "${BLUE}[info]${NC} $*"; }
err() { echo -e "${RED}[error]${NC} $*" >&2; exit 1; }

check_linux() {
    if [[ "$OSTYPE" != linux* ]]; then
        err "This installer only supports Linux. Detected: $OSTYPE"
    fi
}

detect_distro() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        echo "$ID"
    else
        echo "unknown"
    fi
}

install_system_deps() {
    local distro
    distro="$(detect_distro)"
    log "Detected distro: $distro"

    if command -v xvfb-run >/dev/null 2>&1 && command -v openbox >/dev/null 2>&1; then
        log "xvfb and openbox already installed."
        return 0
    fi

    log "Installing system dependencies..."
    case "$distro" in
        debian|ubuntu|deepin|linuxmint|pop)
            sudo apt-get update
            sudo apt-get install -y xvfb openbox
            ;;
        fedora|rhel|centos|rocky|almalinux)
            sudo dnf install -y xorg-x11-server-Xvfb openbox
            ;;
        arch|manjaro|endeavouros)
            sudo pacman -S --noconfirm xorg-server-xvfb openbox
            ;;
        opensuse*|suse*)
            sudo zypper install -y xorg-x11-server-Xvfb openbox
            ;;
        *)
            warn "Unknown distro. Please install xvfb and openbox manually."
            ;;
    esac
}

check_rust() {
    if ! command -v cargo >/dev/null 2>&1; then
        err "Rust toolchain not found. Install from https://rustup.rs/ first."
    fi
}

build_project() {
    log "Building release binary..."
    cd "$PROJECT_DIR"
    cargo build --release
    if [[ ! -x "$BINARY" ]]; then
        err "Build succeeded but binary not found at $BINARY"
    fi
    log "Binary: $BINARY"
}

ensure_python3() {
    if ! command -v python3 >/dev/null 2>&1; then
        err "python3 is required for agent configuration. Please install it."
    fi
}

link_skill() {
    local target_dir="$1"
    mkdir -p "$target_dir"
    if [[ -L "$target_dir/linux-x11-sandbox" ]]; then
        rm "$target_dir/linux-x11-sandbox"
    fi
    ln -sfn "$SKILL_SRC" "$target_dir/linux-x11-sandbox"
    log "Linked skill -> $target_dir/linux-x11-sandbox"
}

register_skills() {
    log "Registering skill for detected agents..."

    # pi
    if [[ -d "$HOME/.pi" ]] || [[ -d "$PROJECT_DIR/.pi" ]]; then
        link_skill "$HOME/.pi/agent/skills"
    else
        info "pi not detected. To register manually: ln -s $SKILL_SRC ~/.pi/agent/skills/linux-x11-sandbox"
    fi

    # Claude Code (project-local)
    if command -v claude >/dev/null 2>&1 || [[ -d "$PROJECT_DIR/.claude" ]]; then
        link_skill "$PROJECT_DIR/.claude/skills"
    else
        info "Claude Code not detected. To register manually: ln -s $SKILL_SRC .claude/skills/linux-x11-sandbox"
    fi

    # Codex
    if command -v codex >/dev/null 2>&1 || [[ -d "$HOME/.codex" ]]; then
        link_skill "$HOME/.codex/skills"
    else
        info "Codex not detected. To register manually: ln -s $SKILL_SRC ~/.codex/skills/linux-x11-sandbox"
    fi

    # Generic .agents
    mkdir -p "$HOME/.agents/skills"
    link_skill "$HOME/.agents/skills"
}

backup_file() {
    local path="$1"
    if [[ -f "$path" && ! -f "$path.lxs-backup" ]]; then
        cp "$path" "$path.lxs-backup"
        log "Backed up $path -> $path.lxs-backup"
    fi
}

update_json_config() {
    local path="$1"
    local key="$2"
    local value="$3"

    backup_file "$path"

    python3 - "$path" "$key" "$value" <<'PY'
import json, os, sys
path, key, value = sys.argv[1], sys.argv[2], sys.argv[3]
os.makedirs(os.path.dirname(path), exist_ok=True)

data = {}
if os.path.exists(path):
    try:
        with open(path) as f:
            data = json.load(f)
    except json.JSONDecodeError:
        print(f"Warning: {path} is malformed; replacing.")

data.setdefault("mcpServers", {})
data["mcpServers"][key] = {"command": value}

with open(path, "w") as f:
    json.dump(data, f, indent=2)
print(f"Updated {path}")
PY
}

register_mcp_servers() {
    log "Registering MCP server for detected agents..."

    # Claude Desktop
    local claude_config="$HOME/.config/claude/claude_desktop_config.json"
    if [[ -d "$HOME/.config/claude" ]] || [[ -f "$claude_config" ]]; then
        ensure_python3
        update_json_config "$claude_config" "linux-x11-sandbox" "$BINARY"
    else
        info "Claude Desktop not detected. Config path: $claude_config"
    fi

    # Cursor
    local cursor_config="$HOME/.cursor/mcp.json"
    if [[ -d "$HOME/.cursor" ]] || [[ -f "$cursor_config" ]]; then
        ensure_python3
        update_json_config "$cursor_config" "linux-x11-sandbox" "$BINARY"
    else
        info "Cursor not detected. Config path: $cursor_config"
    fi

    # Generic instruction for others
    info "For other agents, add this MCP server: { \"command\": \"$BINARY\" }"
}

print_summary() {
    echo
    log "Installation complete."
    echo
    echo "Binary:    $BINARY"
    echo "Skill:     $SKILL_SRC"
    echo
    info "Restart your agent to pick up the new MCP server and skill."
    echo
    echo "Quick test:"
    echo "  $BINARY"
    echo "  Then send JSON-RPC initialize + lxs_display_create."
}

main() {
    check_linux
    install_system_deps
    check_rust
    build_project
    register_skills
    register_mcp_servers
    print_summary
}

main "$@"
