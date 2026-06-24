#!/usr/bin/env bash
# monitor.sh — fleet status + global best across all instances (reads S3 only).
# Usage: tools/deploy/monitor.sh --run-id ID [--profile NAME]
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"

RUN_ID=run1 PROFILE=dev
opts=$(getopt -o h --long help,run-id:,profile: -n monitor.sh -- "$@") \
  || { echo "usage: monitor.sh --run-id ID [--profile NAME]"; exit 1; }
eval set -- "$opts"
while true; do case "$1" in
  --run-id) RUN_ID=$2; shift 2;;
  --profile) PROFILE=$2; shift 2;;
  -h|--help) echo "usage: monitor.sh --run-id ID [--profile NAME]"; exit 0;;
  --) shift; break;;
  *) exit 1;;
esac; done

# shellcheck disable=SC1090
source "$HERE/.state-$RUN_ID"
P="aws --profile $PROFILE --region $REGION"
REPLAY="$HERE/../../target/release/morpion-solitaire"

echo "=== fleet $FLEET_ID ==="
$P ec2 describe-fleets --fleet-ids "$FLEET_ID" \
  --query 'Fleets[0].{state:FleetState,target:TargetCapacitySpecification.TotalTargetCapacity,fulfilled:FulfilledCapacity}' \
  --output table 2>/dev/null || echo "(fleet not found)"

echo "=== running instances (+ ssh) ==="
KEY="${KEY:-smj}"
$P ec2 describe-instances --filters "Name=tag:ms-run,Values=$RUN_ID" \
  "Name=instance-state-name,Values=running" \
  --query 'Reservations[].Instances[].[InstanceId,InstanceType,Placement.AvailabilityZone,PublicIpAddress]' \
  --output text 2>/dev/null | while read -r iid type az ip; do
    printf "  %-19s %-12s %-11s  ssh -i %s.pem ec2-user@%s\n" "$iid" "$type" "$az" "$KEY" "$ip"
  done

echo "=== per-instance best (from S3) ==="
TMP=$(mktemp -d); BEST=0
for pre in $($P s3 ls "s3://$BUCKET/$RUN_ID/inst/" 2>/dev/null | awk '{print $2}'); do
  iid="${pre%/}"
  $P s3 cp "s3://$BUCKET/$RUN_ID/inst/${pre}best.msr" "$TMP/$iid.msr" --only-show-errors 2>/dev/null || continue
  sc=$("$REPLAY" replay "$TMP/$iid.msr" 2>/dev/null | grep -oE 'score: [0-9]+' | grep -oE '[0-9]+' | head -1)
  [ -z "${sc:-}" ] && continue
  printf "  %-20s score=%s\n" "$iid" "$sc"
  if [ "$sc" -gt "$BEST" ]; then BEST=$sc; cp "$TMP/$iid.msr" "$HERE/global-best-$RUN_ID.msr"; fi
done
echo "=== GLOBAL BEST = $BEST  (record to beat: 178) ==="
[ "$BEST" -gt 0 ] && echo "    saved: tools/deploy/global-best-$RUN_ID.msr"
rm -rf "$TMP"
