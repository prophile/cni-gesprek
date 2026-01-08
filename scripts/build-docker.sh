#!/bin/bash

set -e

# Docker Build Script for CNI-Gesprek
# Builds both Ubuntu and Alpine variants with multi-architecture support

REGISTRY="${REGISTRY:-}"
TAG="${TAG:-latest}"
PLATFORMS="${PLATFORMS:-linux/amd64,linux/arm64}"
PUSH="${PUSH:-false}"

echo "Building CNI-Gesprek Docker images..."
echo "Registry: ${REGISTRY:-none}"
echo "Tag: $TAG"
echo "Platforms: $PLATFORMS"
echo "Push: $PUSH"

# Build Ubuntu-based image
echo "Building Ubuntu-based image..."
if [[ -n "$REGISTRY" ]]; then
    IMAGE_NAME="$REGISTRY/cni-gesprek:$TAG"
else
    IMAGE_NAME="cni-gesprek:$TAG"
fi

if [[ "$PUSH" == "true" ]]; then
    docker buildx build \
        --platform "$PLATFORMS" \
        --tag "$IMAGE_NAME" \
        --file docker/Dockerfile \
        --push \
        .
else
    docker buildx build \
        --platform "$PLATFORMS" \
        --tag "$IMAGE_NAME" \
        --file docker/Dockerfile \
        --load \
        .
fi

echo "✓ Built Ubuntu-based image: $IMAGE_NAME"

# Build Alpine-based image
echo "Building Alpine-based image..."
if [[ -n "$REGISTRY" ]]; then
    ALPINE_IMAGE_NAME="$REGISTRY/cni-gesprek:alpine-$TAG"
else
    ALPINE_IMAGE_NAME="cni-gesprek:alpine-$TAG"
fi

if [[ "$PUSH" == "true" ]]; then
    docker buildx build \
        --platform "$PLATFORMS" \
        --tag "$ALPINE_IMAGE_NAME" \
        --file docker/Dockerfile.alpine \
        --push \
        .
else
    docker buildx build \
        --platform "$PLATFORMS" \
        --tag "$ALPINE_IMAGE_NAME" \
        --file docker/Dockerfile.alpine \
        --load \
        .
fi

echo "✓ Built Alpine-based image: $ALPINE_IMAGE_NAME"

echo ""
echo "Docker images built successfully!"
echo "Available images:"
docker images | grep cni-gesprek

if [[ "$PUSH" == "false" ]]; then
    echo ""
    echo "To test the image:"
    echo "  docker run --rm $IMAGE_NAME /usr/local/bin/cni-gesprek --help"
    echo ""
    echo "To push the images:"
    echo "  PUSH=true $0"
fi
