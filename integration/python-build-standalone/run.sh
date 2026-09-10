#!/usr/bin/env bash
# Run only from an isolated PBS checkout prepared with the accompanying patch.
set -euo pipefail

if [[ $# != 2 ]]; then
    echo "usage: $0 /absolute/pbs-checkout /absolute/oriole-bundle" >&2
    exit 2
fi
pbs_checkout=$(realpath "$1")
oriole_bundle=$(realpath "$2")
revision=a4553880293fe9d1bb62747d34ab0e5121d3554f
if [[ $(git -C "$pbs_checkout" rev-parse HEAD) != "$revision" ]]; then
    echo "PBS checkout must be at $revision" >&2
    exit 1
fi
integration_dir=$(cd "$(dirname "$0")" && pwd)
git -C "$pbs_checkout" apply --reverse --check "$integration_dir/pbs-a455388.patch"
test -f "$oriole_bundle/manifest.json"
cd "$pbs_checkout"
PYBUILD_ORIOLE_BUNDLE="$oriole_bundle" uv run --no-dev build.py \
    --target-triple x86_64-unknown-linux-gnu --python cpython-3.12 --options noopt
