#!/bin/bash
set -euo pipefail

CERT_SOURCE_DIR="${HOME}/.config/palette/certs"
CERT_TARGET_DIR="/usr/local/share/ca-certificates/custom"

install_custom_ca_certs() {
    if [[ ! -d "${CERT_SOURCE_DIR}" ]]; then
        echo "No custom cert dir found at ${CERT_SOURCE_DIR}; using system CAs only."
        return
    fi

    shopt -s nullglob
    local cert_files=("${CERT_SOURCE_DIR}"/*.crt "${CERT_SOURCE_DIR}"/*.pem)
    shopt -u nullglob

    if [[ ${#cert_files[@]} -eq 0 ]]; then
        echo "No custom cert files found in ${CERT_SOURCE_DIR}; using system CAs only."
        return
    fi

    echo "Installing ${#cert_files[@]} custom CA cert(s) from ${CERT_SOURCE_DIR}..."
    sudo mkdir -p "${CERT_TARGET_DIR}"

    local cert_file
    for cert_file in "${cert_files[@]}"; do
        local cert_name
        cert_name="$(basename "${cert_file}")"
        cert_name="${cert_name%.*}.crt"
        sudo cp "${cert_file}" "${CERT_TARGET_DIR}/${cert_name}"
    done

    sudo update-ca-certificates
}

install_custom_ca_certs

# Check if Claude Code is installed
if ! command -v claude &> /dev/null; then
    echo "First startup detected. Installing Claude Code using native installer..."

    # Install Claude Code using native installer
    curl -fsSL https://claude.ai/install.sh | bash
    echo "Installation completed."
else
    echo "Claude Code is already installed. Checking for updates..."

    # Update Claude Code to latest version (continue on failure to prevent container crash)
    claude update || echo "[WARNING] claude update failed (exit $?), continuing with current version."
    echo "Update check completed."
fi

# Start Claude Code
exec claude
