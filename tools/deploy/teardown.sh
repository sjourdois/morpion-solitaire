#!/usr/bin/env bash
# teardown.sh — stop a run: delete the fleet (terminate instances), launch
# template, and SG. Keeps the S3 results and the shared bucket + IAM for reuse.
# Usage: tools/deploy/teardown.sh --run-id ID [--profile NAME]
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"

RUN_ID=run1 PROFILE=dev
opts=$(getopt -o h --long help,run-id:,profile: -n teardown.sh -- "$@") \
  || { echo "usage: teardown.sh --run-id ID [--profile NAME]"; exit 1; }
eval set -- "$opts"
while true; do case "$1" in
  --run-id) RUN_ID=$2; shift 2;;
  --profile) PROFILE=$2; shift 2;;
  -h|--help) echo "usage: teardown.sh --run-id ID [--profile NAME]"; exit 0;;
  --) shift; break;;
  *) exit 1;;
esac; done

# shellcheck disable=SC1090
source "$HERE/.state-$RUN_ID"
P="aws --profile $PROFILE --region $REGION"

echo ">> deleting fleet $FLEET_ID (terminating its instances)"
$P ec2 delete-fleets --fleet-ids "$FLEET_ID" --terminate-instances \
  --query 'SuccessfulFleetDeletions[0].CurrentFleetState' --output text 2>&1 || true

echo ">> waiting for instances to terminate..."
for _ in $(seq 1 36); do
  n=$($P ec2 describe-instances --filters "Name=tag:ms-run,Values=$RUN_ID" \
      "Name=instance-state-name,Values=running,pending,stopping,shutting-down" \
      --query 'length(Reservations[].Instances[])' --output text 2>/dev/null || echo 0)
  [ "$n" = "0" ] && { echo "   all terminated"; break; }
  sleep 10
done

echo ">> deleting launch template $LT_NAME"
$P ec2 delete-launch-template --launch-template-name "$LT_NAME" >/dev/null 2>&1 || true
echo ">> deleting SG $SG"
$P ec2 delete-security-group --group-id "$SG" 2>/dev/null || echo "   (SG still in use, retry later)"

echo ">> kept: s3://$BUCKET/$RUN_ID/ results, bucket, IAM ms-fleet-{role,profile}"
rm -f "$HERE/.state-$RUN_ID"
echo ">> done"
