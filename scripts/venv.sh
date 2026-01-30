#!/bin/bash
set -e

{
  cd `git rev-parse --show-toplevel`
  rm -rf venv
  python -mvenv venv
  source venv/bin/activate
  pip install -r requirements.txt
}

{
  cd `git rev-parse --show-toplevel`
  cd tests_external
  rm -rf venv
  python -mvenv venv
  source venv/bin/activate
  pip install -r requirements.txt
}
