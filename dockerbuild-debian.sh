#!/bin/bash

SOLHATUI_VERSION=`cargo pkgid | cut -d \# -f 2`
. docker/config.sh

# Build debs for Debian
DOCKER_IMAGE_NAME=${VENDOR}/build_debs_${PROJECT}

docker build -t ${DOCKER_IMAGE_NAME} --build-arg SOLHATUI_VERSION=$SOLHATUI_VERSION -f docker/Dockerfile.debian . 

CONTAINER_ID=$(docker run -d ${DOCKER_IMAGE_NAME})

if [ ! -d target/release ]; then
    mkdir -p target/release
fi

docker cp ${CONTAINER_ID}:/build/target/debian/ target/release/

docker rm -f ${CONTAINER_ID}  || true
docker rmi -f ${DOCKER_IMAGE_NAME} || true

