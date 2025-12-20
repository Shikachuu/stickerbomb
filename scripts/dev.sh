#!/usr/bin/env bash
set -euo pipefail

CLUSTER_NAME="stickerbomb"
REGISTRY_PORT="5555"

echo "🚀 Starting stickerbomb development environment..."

if ! k3d cluster list | grep -q "${CLUSTER_NAME}"; then
    echo "📦 Creating k3d cluster: ${CLUSTER_NAME}"
    k3d cluster create "${CLUSTER_NAME}" \
        --registry-create "k3d-${CLUSTER_NAME}-registry:${REGISTRY_PORT}" \
        --servers 1 \
        --agents 0 \
        --wait
else
    echo "✅ k3d cluster ${CLUSTER_NAME} already exists"
fi

echo "📝 Generating CRDs..."
mise run generate-crds

echo "🎯 Starting Tilt..."
tilt up
