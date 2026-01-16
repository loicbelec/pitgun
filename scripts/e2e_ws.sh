#!/usr/bin/env bash
set -euo pipefail

"$(dirname "$0")/contract.sh"
"$(dirname "$0")/emit_ws.sh"
