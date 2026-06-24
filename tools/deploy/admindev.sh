#!/usr/bin/env bash
# Audited wrapper for every privileged AWS call. ALL admindev (AdministratorAccess)
# actions go through here so they are logged and reviewable. Plain `dev` calls do
# NOT use this — only the few admin actions (IAM, PassRole-bearing launches) do.
#
# Usage: deploy/admindev.sh <aws-args...>   e.g.  deploy/admindev.sh iam get-role ...
set -uo pipefail
PROFILE="${MS_ADMIN_PROFILE:-admindev}"
LOG="$(cd "$(dirname "$0")" && pwd)/admindev-audit.log"
echo "$(date -Is)  aws --profile $PROFILE $*" >> "$LOG"
aws --profile "$PROFILE" "$@"
rc=$?
echo "$(date -Is)  -> exit $rc" >> "$LOG"
exit $rc
