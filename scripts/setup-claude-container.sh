#!/bin/bash

# Script to setup Claude container environment

set -e

main() {
    check_prerequisites
    mkdir -p claude.local/.claude claude.local/.npm-global claude.local/.local
    copy_host_credentials
    setup_claude_config
    setup_bash_history
    echo "Setup completed successfully!"
    echo "Local Claude runtime files prepared under claude.local/"
}

check_prerequisites() {
    # Check if git config uses XDG location
    if [ ! -f ~/.config/git/config ]; then
        echo "Error: ~/.config/git/config not found."
        echo "This container mounts ~/.config/git for git configuration."
        echo ""
        echo "Please migrate from ~/.gitconfig to ~/.config/git/config:"
        echo "  mkdir -p ~/.config/git"
        echo "  mv ~/.gitconfig ~/.config/git/config"
        exit 1
    fi
}

setup_claude_config() {

    # Check if claude.local/.claude.json already exists
    if [ -f claude.local/.claude.json ]; then
        echo "claude.local/.claude.json already exists. Skipping Claude config setup."
        return
    fi

    cat > claude.local/.claude.json <<'EOF'
{"hasCompletedOnboarding":true,"bypassPermissionsModeAccepted":true,"projects":{}}
EOF

    echo "Successfully created claude.local/.claude.json with empty project history"
}

copy_host_credentials() {
    local src="${HOME}/.claude/.credentials.json"
    local dst="claude.local/.claude/.credentials.json"

    if [[ -f "${dst}" ]]; then
        return
    fi

    if [[ -f "${src}" ]]; then
        echo "Copying ${src} to ${dst} so Linux credentials persist inside claude.local."
        cp "${src}" "${dst}"
        chmod 600 "${dst}"
    fi
}

setup_bash_history() {
    # Check if .bash_history already exists
    if [ -f claude.local/.bash_history ]; then
        echo "claude.local/.bash_history already exists. Skipping bash history setup."
        return
    fi

    # Create empty .bash_history file for Docker volume mount
    echo "Creating empty .bash_history file..."
    touch claude.local/.bash_history

    echo "Created claude.local/.bash_history for Docker history persistence"
}

main
