#!/usr/bin/env bash
# Remaining agy quota (host agy, owner's main Google account).
set -u
agy -p "/usage" --print-timeout 60s 2>&1 | grep -E "Limit|%" || echo "(no answer; is agy logged in?)"
