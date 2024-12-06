#!/bin/bash

set -e

[[ ${UID} == "0" ]] || SUDO="sudo"

function install_essential_deps_linux() {
    $SUDO bash -c '
        apt update && apt install -y \
            ca-certificates \
            curl \
            git \
            gnupg \
            jq \
            libssl-dev \
            lsb-release \
            pkg-config \
            ripgrep \
            software-properties-common \
            zstd \
            wget \
            lld
  '
}

function setup_llvm_deps() {
    case "$(uname)" in
    Darwin)
        brew update
        brew install llvm@19
        ;;
    Linux)
        $SUDO bash -c 'apt update && apt-get install -y \
            libgmp3-dev \
            llvm-19 \
            libmlir-19-dev \
            libpolly-19-dev \
            libzstd-dev \
            mlir-19-tools
        '
        # Add LLVM to PATH by creating a file in profile.d
        echo 'export PATH=/usr/lib/llvm-19/bin:$PATH' | $SUDO tee /etc/profile.d/llvm19.sh
        $SUDO chmod +x /etc/profile.d/llvm19.sh
        # Source the file immediately
        source /etc/profile.d/llvm19.sh
        ;;
    *)
        echo "Error: Unsupported operating system"
        exit 1
        ;;
    esac
}

function main() {
    [ "$(uname)" = "Linux" ] && install_essential_deps_linux
    setup_llvm_deps
    echo "LLVM dependencies installed successfully."
}

main "$@"
