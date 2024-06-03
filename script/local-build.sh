#!/bin/bash

show_usage()
{
  echo ""
  echo "Usage:   $0 DIST DSC"
  echo ""
  echo "Example: ./local-build.sh jammy path/to/ibus_1.5.28-3sil1~jammy.dsc"
  exit 1
}

if [ "$1" == "--help" ]; then
  show_usage 0
fi

if [[ $# -ne 2 ]]; then
  show_usage 1
fi

WORK_DIR="$(pwd)"
DIST="$1"
DSC="$(basename "$2")"
SOURCE_DIR="$(readlink -f "$(dirname "$2")")"
DSC_DIR="$(realpath --relative-to="${WORK_DIR}" "${SOURCE_DIR}")"

if [[ "$SOURCE_DIR" != "$WORK_DIR"* ]]; then
  echo "Source directory ($SOURCE_DIR) has to be a subdirectory of the current workdir!"
  exit 2
fi

SCRIPT_DIR="$(readlink -f "$(dirname "$0")")"
IMAGE_NAME="sillsdev/sil-ubuntu-${DIST}-amd64-true-true"

cd "$SCRIPT_DIR" || exit

IMAGE_ID=$(docker images --quiet "${IMAGE_NAME}")

if [ -z "$IMAGE_ID" ]; then
  echo -e "\e[0;35mBuilding docker image for ${DIST}\e[0m"
  docker build --build-arg DIST="${DIST}" --build-arg PLATFORM=amd64 -t "${IMAGE_NAME}" .
fi

echo -e "\e[0;35mBuilding binary image for ${DIST}\e[0m"
docker run -v "${WORK_DIR}":/source --env INPUT_DISTRIBUTION=ubuntu \
  --env INPUT_DIST="$DIST" --env INPUT_PLATFORM=amd64 \
  --env INPUT_SOURCEPACKAGE="${DSC}" --env INPUT_SOURCE_DIR="${DSC_DIR}" \
  --env INPUT_RESULT_DIR --env INPUT_ENABLE_LLSO --env INPUT_ENABLE_PSO \
  --env INPUT_DEB_FULLNAME --env INPUT_DEB_EMAIL --env INPUT_PRERELEASE_TAG \
  -i -t -w /source --platform=amd64 "${IMAGE_NAME}"