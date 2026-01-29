#!/bin/bash
set -e

pushd app_rs/
rustfmt `find src -type f -name '*rs'`
popd

source venv/bin/activate
cd app/
black .
