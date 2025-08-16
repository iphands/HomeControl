#!/bin/bash
podman build -t homectrl .

podman run \
  --rm \
  -ti \
  --net=host \
  -e HOME_TOKEN \
  -v /etc/letsencrypt/live/home.ahands.org/:/ssl/home.ahands.org \
  -v /etc/letsencrypt/archive/home.ahands.org/:/archive/home.ahands.org \
  homectrl
