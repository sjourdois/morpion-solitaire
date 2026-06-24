#!/usr/bin/env bash
# ============================================================================
# deploy.sh — launch (or grow) the Morpion Solitaire record-hunt spot fleet.
#
# Uploads the headless aarch64 binary + run-unit.sh to S3, (re)creates a launch
# template, and creates an EC2 Fleet (type=maintain) of spot instances across AZs
# and instance types (capacity-optimized + capacity-rebalancing). The fleet keeps
# TotalTargetCapacity alive, auto-replacing interrupted spot instances; each
# instance hunts independently and checkpoints its best to S3 (see run-unit.sh).
#
# Everything here uses the plain `dev` profile (PassRole on ms-fleet-role was
# granted to it). No admin profile needed.
#
# ============================================================================
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"

usage() {
  cat <<EOF
Usage: deploy.sh [options]
  --region   REGION   AWS region                       (eu-west-1)
  --run-id   ID        run name / S3 prefix / tag        (run1)
  --capacity N         spot instances to maintain        (4)
  --variant  V         5T|5D|4T|4D                        (5T)
  --level    L         NRPA level (nrpa only)            (4)
  --algo     A         nrpa|perturbation                  (nrpa)
  --xover    R         crossover rate (perturbation)      (0)
  --types    "t1 t2"  instance types to mix              (c7g.xlarge c8g.xlarge)
  --key      NAME      SSH key pair (must exist in region) (smj)
  --profile  NAME      AWS profile                        (dev)
  --bucket   NAME      S3 bucket            (ms-recordhunt-<account>)
  --ami      ID         AMI                  (latest AL2023 arm64)
  --bin      PATH      aarch64 binary to upload
EOF
}

REGION=eu-west-1 RUN_ID=run1 CAPACITY=4 VARIANT=5T LEVEL=4 ALGO=nrpa XOVER=0
TYPES="c7g.xlarge c8g.xlarge" KEY=smj PROFILE=dev BUCKET="" AMI="" BIN=""
opts=$(getopt -o h --long help,region:,run-id:,capacity:,variant:,level:,algo:,xover:,types:,key:,profile:,bucket:,ami:,bin: -n deploy.sh -- "$@") || { usage; exit 1; }
eval set -- "$opts"
while true; do case "$1" in
  --region) REGION=$2; shift 2;;
  --run-id) RUN_ID=$2; shift 2;;
  --capacity) CAPACITY=$2; shift 2;;
  --variant) VARIANT=$2; shift 2;;
  --level) LEVEL=$2; shift 2;;
  --algo) ALGO=$2; shift 2;;
  --xover) XOVER=$2; shift 2;;
  --types) TYPES=$2; shift 2;;
  --key) KEY=$2; shift 2;;
  --profile) PROFILE=$2; shift 2;;
  --bucket) BUCKET=$2; shift 2;;
  --ami) AMI=$2; shift 2;;
  --bin) BIN=$2; shift 2;;
  -h|--help) usage; exit 0;;
  --) shift; break;;
  *) usage; exit 1;;
esac; done

P="aws --profile $PROFILE --region $REGION"
BIN="${BIN:-$HERE/../../target/aarch64-unknown-linux-gnu/release/morpion-solitaire}"
ACCT="$($P sts get-caller-identity --query Account --output text)"
BUCKET="${BUCKET:-ms-recordhunt-$ACCT}"
AMI="${AMI:-$($P ssm get-parameters --names /aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64 --query 'Parameters[0].Value' --output text)}"

echo ">> region=$REGION run=$RUN_ID capacity=$CAPACITY types=[$TYPES] bucket=$BUCKET ami=$AMI"
[ -f "$BIN" ] || { echo "!! binary not found: $BIN  (build it: see tools/deploy/README.md)"; exit 1; }

echo ">> uploading artifacts to s3://$BUCKET/$RUN_ID/bin/"
$P s3 cp "$BIN" "s3://$BUCKET/$RUN_ID/bin/morpion-solitaire" --only-show-errors
$P s3 cp "$HERE/run-unit.sh" "s3://$BUCKET/$RUN_ID/bin/run-unit.sh" --only-show-errors

# --- security group: egress only (hands-off; no inbound needed) --------------
VPC="$($P ec2 describe-vpcs --filters Name=isDefault,Values=true --query 'Vpcs[0].VpcId' --output text)"
SG="$($P ec2 describe-security-groups --filters Name=group-name,Values=ms-fleet-sg Name=vpc-id,Values=$VPC --query 'SecurityGroups[0].GroupId' --output text 2>/dev/null || true)"
if [ "$SG" = "None" ] || [ -z "$SG" ]; then
  SG="$($P ec2 create-security-group --group-name ms-fleet-sg --description 'MS fleet' --vpc-id "$VPC" --query GroupId --output text)"
  echo ">> created SG $SG"
fi
# Allow SSH from the deployer's current public IP (idempotent) so you can log in.
MYIP="$(curl -s https://checkip.amazonaws.com)"
$P ec2 authorize-security-group-ingress --group-id "$SG" --protocol tcp --port 22 \
  --cidr "${MYIP}/32" >/dev/null 2>&1 && echo ">> SSH ingress for ${MYIP}/32" || true

# --- user-data (bootstrap with this run's params), base64 --------------------
UD="$(sed -e "s|@@BUCKET@@|$BUCKET|" -e "s|@@RUN@@|$RUN_ID|" -e "s|@@VARIANT@@|$VARIANT|" \
        -e "s|@@LEVEL@@|$LEVEL|" -e "s|@@ALGO@@|$ALGO|" -e "s|@@XOVER@@|$XOVER|" "$HERE/bootstrap.sh" \
      | base64 -w0)"

# --- launch template (recreate each deploy so params are fresh) --------------
LT_NAME="ms-fleet-$RUN_ID"
$P ec2 delete-launch-template --launch-template-name "$LT_NAME" >/dev/null 2>&1 || true
LT_DATA=$(cat <<JSON
{
  "ImageId": "$AMI",
  "KeyName": "$KEY",
  "IamInstanceProfile": {"Name": "ms-fleet-profile"},
  "SecurityGroupIds": ["$SG"],
  "UserData": "$UD",
  "TagSpecifications": [
    {"ResourceType": "instance", "Tags": [
      {"Key": "Name", "Value": "ms-fleet-$RUN_ID"},
      {"Key": "ms-run", "Value": "$RUN_ID"}
    ]}
  ]
}
JSON
)
LT_ID="$($P ec2 create-launch-template --launch-template-name "$LT_NAME" \
  --launch-template-data "$LT_DATA" --query 'LaunchTemplate.LaunchTemplateId' --output text)"
echo ">> launch template $LT_ID ($LT_NAME)"

# --- overrides: every instance type x every default-VPC AZ subnet ------------
SUBNETS="$($P ec2 describe-subnets --filters Name=vpc-id,Values=$VPC Name=default-for-az,Values=true --query 'Subnets[].SubnetId' --output text)"
OVERRIDES=""
for t in $TYPES; do for s in $SUBNETS; do
  OVERRIDES="$OVERRIDES{\"InstanceType\":\"$t\",\"SubnetId\":\"$s\"},"
done; done
OVERRIDES="[${OVERRIDES%,}]"

# --- create the maintained spot fleet ---------------------------------------
FLEET_CFG=$(cat <<JSON
{
  "Type": "maintain",
  "TargetCapacitySpecification": {"TotalTargetCapacity": $CAPACITY, "DefaultTargetCapacityType": "spot"},
  "SpotOptions": {
    "AllocationStrategy": "capacity-optimized",
    "MaintenanceStrategies": {"CapacityRebalance": {"ReplacementStrategy": "launch"}}
  },
  "LaunchTemplateConfigs": [
    {"LaunchTemplateSpecification": {"LaunchTemplateId": "$LT_ID", "Version": "\$Latest"}, "Overrides": $OVERRIDES}
  ],
  "TagSpecifications": [{"ResourceType": "fleet", "Tags": [{"Key": "ms-run", "Value": "$RUN_ID"}]}]
}
JSON
)
# First EC2 Fleet in an account triggers async creation of its service-linked role;
# the first create-fleet then fails with ServiceLinkedRoleCreationInProgress. Retry.
ERR=$(mktemp); trap 'rm -f "$ERR"' EXIT
FLEET_ID=""
for attempt in 1 2 3 4 5 6; do
  if FLEET_ID="$($P ec2 create-fleet --cli-input-json "$FLEET_CFG" --query 'FleetId' --output text 2>"$ERR")" \
     && [ -n "$FLEET_ID" ] && [ "$FLEET_ID" != "None" ]; then
    break
  fi
  if grep -qi ServiceLinkedRole "$ERR" 2>/dev/null; then
    echo ">> EC2 Fleet service-linked role still provisioning — retry $attempt/6 in 15s"
    sleep 15; FLEET_ID=""
  else
    echo "!! create-fleet failed:"; cat "$ERR"; exit 1
  fi
done
[ -n "$FLEET_ID" ] && [ "$FLEET_ID" != "None" ] || { echo "!! create-fleet did not succeed"; exit 1; }
echo ">> FLEET $FLEET_ID — maintaining $CAPACITY spot instances"

# --- save state for monitor/teardown ----------------------------------------
cat > "$HERE/.state-$RUN_ID" <<EOF
REGION=$REGION
RUN_ID=$RUN_ID
BUCKET=$BUCKET
FLEET_ID=$FLEET_ID
LT_NAME=$LT_NAME
SG=$SG
KEY=$KEY
EOF
echo ">> state saved: tools/deploy/.state-$RUN_ID"
echo ">> monitor:  tools/deploy/monitor.sh --run-id $RUN_ID"
echo ">> teardown: tools/deploy/teardown.sh --run-id $RUN_ID"
