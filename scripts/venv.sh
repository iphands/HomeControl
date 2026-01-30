#!/bin/bash
set -e

function gentoo() {
  cd `git rev-parse --show-toplevel`
  rm -rf venv_gentoo
  python -mvenv venv_gentoo
  source venv_gentoo/bin/activate
  pip install -r requirements.txt
}

function fedora() {
  cd `git rev-parse --show-toplevel`
  rm -rf venv_fedora
  python -mvenv venv_fedora
  source venv_fedora/bin/activate
  pip install -r requirements.txt
}

if [[ -f /etc/redhat-release ]]
then
  fedora
else
  gentoo
fi
