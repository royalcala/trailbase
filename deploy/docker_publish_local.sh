#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'EOF'
Publish TrailBase Docker image from a local machine using buildx.

Environment variables:
  REGISTRY      Docker registry host (default: ghcr.io)
  IMAGE_NAME    Repository/image name (default: royalcala/trailbase)
  TAGS          Comma-separated tags (default: latest,sha-<short_commit>)
  PLATFORMS     Comma-separated platforms (default: linux/amd64,linux/arm64)
  BUILDER       buildx builder name (default: trailbase-local-builder)
  CONTEXT       Build context (default: .)
  DOCKERFILE    Dockerfile path (default: ./Dockerfile)
  PUSH          true/false. true pushes to registry, false loads locally.

Examples:
  ./deploy/docker_publish_local.sh
  TAGS=v0.27.9,latest ./deploy/docker_publish_local.sh
  IMAGE_NAME=trailbaseio/trailbase TAGS=nightly ./deploy/docker_publish_local.sh
EOF
  exit 0
fi

REGISTRY="${REGISTRY:-ghcr.io}"
IMAGE_NAME="${IMAGE_NAME:-royalcala/trailbase}"
SHORT_SHA="$(git rev-parse --short HEAD)"
TAGS="${TAGS:-latest,sha-${SHORT_SHA}}"
PLATFORMS="${PLATFORMS:-linux/amd64,linux/arm64}"
BUILDER="${BUILDER:-trailbase-local-builder}"
CONTEXT="${CONTEXT:-.}"
DOCKERFILE="${DOCKERFILE:-./Dockerfile}"
PUSH="${PUSH:-true}"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker not found in PATH" >&2
  exit 1
fi

if ! docker buildx inspect "${BUILDER}" >/dev/null 2>&1; then
  docker buildx create --name "${BUILDER}" --use >/dev/null
else
  docker buildx use "${BUILDER}" >/dev/null
fi

docker buildx inspect --bootstrap >/dev/null

IFS=',' read -r -a TAG_ARRAY <<<"${TAGS}"
TAG_ARGS=()
for tag in "${TAG_ARRAY[@]}"; do
  clean_tag="$(echo "${tag}" | xargs)"
  [[ -z "${clean_tag}" ]] && continue
  TAG_ARGS+=("-t" "${REGISTRY}/${IMAGE_NAME}:${clean_tag}")
done

if [[ "${#TAG_ARGS[@]}" -eq 0 ]]; then
  echo "No valid tags resolved from TAGS=${TAGS}" >&2
  exit 1
fi

echo "Publishing image from local machine"
echo "  Registry:   ${REGISTRY}"
echo "  Image:      ${IMAGE_NAME}"
echo "  Tags:       ${TAGS}"
echo "  Platforms:  ${PLATFORMS}"
echo "  Dockerfile: ${DOCKERFILE}"

BUILD_ARGS=(
  buildx build
  --file "${DOCKERFILE}"
  --platform "${PLATFORMS}"
  "${TAG_ARGS[@]}"
  "${CONTEXT}"
)

if [[ "${PUSH}" == "true" ]]; then
  BUILD_ARGS=("${BUILD_ARGS[@]}" --push)
else
  BUILD_ARGS=("${BUILD_ARGS[@]}" --load)
fi

set -x
docker "${BUILD_ARGS[@]}"
