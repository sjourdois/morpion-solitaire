#!/usr/bin/env bash
#
# coldrun.sh — provision a cold-start 5T record-hunt fleet on AWS spot.
#
# Cold START ONLY: blank NRPA from the empty cross, no neural prior, no known
# record seed (no --prior / no --from). The fleet's value is the *global max*
# across many independent island pools (diversity), checkpointed to S3 and
# spot-reclaim-safe via auto-resume.
#
# Lifecycle (subcommands):
#   build   cross-compile the headless aarch64 binary (cold-pure: no candle)
#   setup   create the S3 bucket, IAM role+instance-profile, and launch template
#   launch  create the EC2 spot Fleet (capacity-optimized, multi-AZ, maintain)
#   status  list running instances + the best score seen so far in S3
#   fetch   sync the run outputs from S3 locally and print the best games
#   down    delete the fleet and terminate its instances (keeps the bucket)
#   nuke    down + delete the IAM role, launch template, and bucket
#
# Requirements: awscli v2, a working `--profile` (SSO ok), rustup target
# aarch64-unknown-linux-gnu, and the Debian cross linker (apt: gcc-aarch64-linux-gnu).
# This script NEVER runs by itself — every subcommand is explicit and the spot
# fleet keeps running (and billing) until `down`. Read the cost note in the README.
#
set -euo pipefail

# ----------------------------------------------------------------------------
# Config — override via env or edit here.
# ----------------------------------------------------------------------------
PROFILE="${PROFILE:-dev}"                 # aws CLI profile (SSO)
REGION="${REGION:-eu-west-3}"             # eu-west-3 = the documented cheap spot region
RUN_NAME="${RUN_NAME:-cold-l4}"           # names the S3 prefix + instance tags
FLEET_SIZE="${FLEET_SIZE:-4}"             # number of spot instances
INSTANCE_TYPES="${INSTANCE_TYPES:-c7g.16xlarge}"  # space-separated; capacity-optimized picks among them
BUCKET="${BUCKET:-}"                      # default: morpion-coldrun-<accountid> (computed in setup)
LOCAL_FETCH_DIR="${LOCAL_FETCH_DIR:-./coldrun-results}"

# The cold-pure search each instance runs. Blank NRPA L4, throughput-on, no prior,
# no seed. `--run-dir` gathers best.msr + checkpoint + progress.log under one dir.
SEARCH_ARGS="${SEARCH_ARGS:---variant 5T search --algo nrpa --level 4 --no-symmetry}"

# Derived / fixed.
AWS=(aws --profile "$PROFILE" --region "$REGION")
ROLE_NAME="morpion-coldrun-role"
PROFILE_NAME="morpion-coldrun-profile"
LT_NAME="morpion-coldrun-lt-${RUN_NAME}"
TARGET="aarch64-unknown-linux-gnu"
BIN="target/${TARGET}/release/morpion-solitaire"

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }
acct() { "${AWS[@]}" sts get-caller-identity --query Account --output text; }
bucket() { echo "${BUCKET:-morpion-coldrun-$(acct)}"; }

# ----------------------------------------------------------------------------
# build — headless aarch64 binary (cold-pure: --no-default-features drops gui AND
# candle/neural, since cold start loads no prior → smaller, faster build).
# ----------------------------------------------------------------------------
cmd_build() {
  command -v aarch64-linux-gnu-gcc >/dev/null \
    || die "missing cross linker — apt install gcc-aarch64-linux-gnu"
  rustup target add "$TARGET" >/dev/null 2>&1 || true
  say "building $BIN (cold-pure, glibc)"
  CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    cargo build --release -p morpion-solitaire \
      --no-default-features --target "$TARGET"
  ls -lh "$BIN"
}

# ----------------------------------------------------------------------------
# setup — bucket, IAM (S3 + SSM Session Manager for shell access), launch template.
# Idempotent: re-running updates the launch template's user-data/AMI.
# ----------------------------------------------------------------------------
cmd_setup() {
  [[ -f "$BIN" ]] || die "binary not built — run: $0 build"
  local b; b="$(bucket)"

  say "S3 bucket: $b"
  "${AWS[@]}" s3 mb "s3://$b" 2>/dev/null || true
  say "uploading binary → s3://$b/bin/morpion-solitaire"
  "${AWS[@]}" s3 cp "$BIN" "s3://$b/bin/morpion-solitaire" --quiet

  # IAM role: let instances read/write the bucket and be reachable via SSM.
  if ! "${AWS[@]}" iam get-role --role-name "$ROLE_NAME" >/dev/null 2>&1; then
    say "creating IAM role $ROLE_NAME"
    "${AWS[@]}" iam create-role --role-name "$ROLE_NAME" \
      --assume-role-policy-document '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}' >/dev/null
    "${AWS[@]}" iam put-role-policy --role-name "$ROLE_NAME" --policy-name s3-run \
      --policy-document "{\"Version\":\"2012-10-17\",\"Statement\":[{\"Effect\":\"Allow\",\"Action\":[\"s3:GetObject\",\"s3:PutObject\",\"s3:ListBucket\"],\"Resource\":[\"arn:aws:s3:::$b\",\"arn:aws:s3:::$b/*\"]}]}" >/dev/null
    "${AWS[@]}" iam attach-role-policy --role-name "$ROLE_NAME" \
      --policy-arn arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore >/dev/null
    "${AWS[@]}" iam create-instance-profile --instance-profile-name "$PROFILE_NAME" >/dev/null
    "${AWS[@]}" iam add-role-to-instance-profile \
      --instance-profile-name "$PROFILE_NAME" --role-name "$ROLE_NAME" >/dev/null
    say "waiting for the instance profile to propagate (~10s)"; sleep 12
  fi

  # Latest AL2023 arm64 AMI (via the public SSM parameter).
  local ami
  ami="$("${AWS[@]}" ssm get-parameter \
    --name /aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64 \
    --query Parameter.Value --output text)"
  say "AL2023 arm64 AMI: $ami"

  # User-data: pull binary, resume from S3 if a checkpoint exists, run, and sync
  # results to S3 every 5 min + once more on a spot interruption notice.
  local userdata b64
  userdata="$(cat <<EOF
#!/bin/bash
set -e
B=$b ; RUN=$RUN_NAME ; REGION=$REGION
command -v aws >/dev/null || dnf install -y awscli-2 || { curl -s "https://awscli.amazonaws.com/awscli-exe-linux-aarch64.zip" -o /tmp/a.zip && (cd /tmp && unzip -q a.zip && ./aws/install); }
TOK=\$(curl -sX PUT "http://169.254.169.254/latest/api/token" -H "X-aws-ec2-metadata-token-ttl-seconds: 600")
IID=\$(curl -s -H "X-aws-ec2-metadata-token: \$TOK" http://169.254.169.254/latest/meta-data/instance-id)
mkdir -p /data/run
aws s3 cp s3://\$B/bin/morpion-solitaire /usr/local/bin/morpion-solitaire --region \$REGION
chmod +x /usr/local/bin/morpion-solitaire
# Resume this instance's own checkpoint if we have one from a prior life.
aws s3 sync s3://\$B/runs/\$RUN/\$IID/ /data/run/ --region \$REGION 2>/dev/null || true
RESUME=""
[ -f /data/run/search-checkpoint-nrpa.msc ] && RESUME="--resume /data/run/search-checkpoint-nrpa.msc"
nohup /usr/local/bin/morpion-solitaire $SEARCH_ARGS \\
  --run-dir /data/run --checkpoint-interval 5m \$RESUME > /data/run/stdout.log 2>&1 &
SEARCH=\$!
# Periodic checkpoint sync + spot-interruption flush.
while kill -0 \$SEARCH 2>/dev/null; do
  aws s3 sync /data/run s3://\$B/runs/\$RUN/\$IID/ --region \$REGION --only-show-errors || true
  CODE=\$(curl -s -o /dev/null -w '%{http_code}' -H "X-aws-ec2-metadata-token: \$TOK" http://169.254.169.254/latest/meta-data/spot/instance-action || echo 000)
  if [ "\$CODE" = "200" ]; then
    aws s3 sync /data/run s3://\$B/runs/\$RUN/\$IID/ --region \$REGION --only-show-errors || true
    break
  fi
  sleep 300
done
EOF
)"
  b64="$(printf '%s' "$userdata" | base64 -w0 2>/dev/null || printf '%s' "$userdata" | base64)"

  say "creating/updating launch template $LT_NAME"
  local lt_data
  lt_data="$(cat <<JSON
{"ImageId":"$ami",
 "IamInstanceProfile":{"Name":"$PROFILE_NAME"},
 "UserData":"$b64",
 "TagSpecifications":[{"ResourceType":"instance","Tags":[{"Key":"Name","Value":"morpion-$RUN_NAME"},{"Key":"morpion-run","Value":"$RUN_NAME"}]}]}
JSON
)"
  if "${AWS[@]}" ec2 describe-launch-templates --launch-template-names "$LT_NAME" >/dev/null 2>&1; then
    "${AWS[@]}" ec2 create-launch-template-version --launch-template-name "$LT_NAME" \
      --launch-template-data "$lt_data" --query 'LaunchTemplateVersion.VersionNumber' --output text
  else
    "${AWS[@]}" ec2 create-launch-template --launch-template-name "$LT_NAME" \
      --launch-template-data "$lt_data" >/dev/null
  fi
  say "setup done. launch with: $0 launch"
}

# ----------------------------------------------------------------------------
# launch — EC2 Fleet, spot, capacity-optimized, spread across the default VPC's
# subnets (multi-AZ). Type=maintain so reclaimed instances are replaced.
# ----------------------------------------------------------------------------
cmd_launch() {
  local subnets overrides it sn
  subnets="$("${AWS[@]}" ec2 describe-subnets \
    --filters Name=default-for-az,Values=true \
    --query 'Subnets[].SubnetId' --output text)"
  [[ -n "$subnets" ]] || die "no default-VPC subnets found in $REGION"

  overrides=""
  for it in $INSTANCE_TYPES; do
    for sn in $subnets; do
      overrides="$overrides {\"InstanceType\":\"$it\",\"SubnetId\":\"$sn\"},"
    done
  done
  overrides="[${overrides%,}]"

  local cfg
  cfg="$(cat <<JSON
[{"LaunchTemplateSpecification":{"LaunchTemplateName":"$LT_NAME","Version":"\$Latest"},"Overrides":$overrides}]
JSON
)"
  say "launching fleet: $FLEET_SIZE × [$INSTANCE_TYPES] spot, capacity-optimized, multi-AZ"
  "${AWS[@]}" ec2 create-fleet \
    --type maintain \
    --target-capacity-specification "TotalTargetCapacity=$FLEET_SIZE,DefaultTargetCapacityType=spot" \
    --spot-options 'AllocationStrategy=capacity-optimized' \
    --launch-template-configs "$cfg" \
    --tag-specifications "ResourceType=fleet,Tags=[{Key=morpion-run,Value=$RUN_NAME}]" \
    --query 'FleetId' --output text
  say "fleet created. check: $0 status   |   stop: $0 down"
}

# ----------------------------------------------------------------------------
# status / fetch / down / nuke
# ----------------------------------------------------------------------------
cmd_status() {
  say "running instances (run=$RUN_NAME)"
  "${AWS[@]}" ec2 describe-instances \
    --filters "Name=tag:morpion-run,Values=$RUN_NAME" "Name=instance-state-name,Values=running,pending" \
    --query 'Reservations[].Instances[].[InstanceId,InstanceType,Placement.AvailabilityZone,InstanceLifecycle]' \
    --output table || true
  local b; b="$(bucket)"
  say "per-instance run prefixes in S3 (pull them with: $0 fetch)"
  "${AWS[@]}" s3 ls "s3://$b/runs/$RUN_NAME/" || true
}

cmd_fetch() {
  local b; b="$(bucket)"
  mkdir -p "$LOCAL_FETCH_DIR"
  say "syncing s3://$b/runs/$RUN_NAME/ → $LOCAL_FETCH_DIR"
  "${AWS[@]}" s3 sync "s3://$b/runs/$RUN_NAME/" "$LOCAL_FETCH_DIR" --only-show-errors
  # The best score per instance = the highest score logged in its progress.log.
  say "best score per instance (from progress.log):"
  find "$LOCAL_FETCH_DIR" -name 'progress.log' | while read -r f; do
    best="$(grep -oiE 'score[=: ]+[0-9]+' "$f" 2>/dev/null | grep -oE '[0-9]+' | sort -n | tail -1)"
    printf '  %s  best=%s\n' "$(dirname "$f")" "${best:-?}"
  done
  echo "candidate games are the best.msr files — verify one with:  morpion-solitaire replay <best.msr>"
}

cmd_down() {
  say "terminating fleet instances (run=$RUN_NAME)"
  local ids
  ids="$("${AWS[@]}" ec2 describe-instances \
    --filters "Name=tag:morpion-run,Values=$RUN_NAME" "Name=instance-state-name,Values=running,pending" \
    --query 'Reservations[].Instances[].InstanceId' --output text)"
  # Delete the fleet(s) so 'maintain' stops replacing instances, then terminate.
  local fleets
  fleets="$("${AWS[@]}" ec2 describe-fleets \
    --query "Fleets[?contains(Tags[?Key=='morpion-run'].Value, '$RUN_NAME')].FleetId" \
    --output text 2>/dev/null || true)"
  [[ -n "$fleets" ]] && "${AWS[@]}" ec2 delete-fleets --fleet-ids $fleets --terminate-instances >/dev/null || true
  [[ -n "$ids" ]] && "${AWS[@]}" ec2 terminate-instances --instance-ids $ids >/dev/null || true
  say "fleet down. S3 results kept (s3://$(bucket)/runs/$RUN_NAME/). Run: $0 fetch"
}

cmd_nuke() {
  cmd_down || true
  say "deleting launch template, IAM, and bucket"
  "${AWS[@]}" ec2 delete-launch-template --launch-template-name "$LT_NAME" >/dev/null 2>&1 || true
  "${AWS[@]}" iam remove-role-from-instance-profile --instance-profile-name "$PROFILE_NAME" --role-name "$ROLE_NAME" >/dev/null 2>&1 || true
  "${AWS[@]}" iam delete-instance-profile --instance-profile-name "$PROFILE_NAME" >/dev/null 2>&1 || true
  "${AWS[@]}" iam delete-role-policy --role-name "$ROLE_NAME" --policy-name s3-run >/dev/null 2>&1 || true
  "${AWS[@]}" iam detach-role-policy --role-name "$ROLE_NAME" --policy-arn arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore >/dev/null 2>&1 || true
  "${AWS[@]}" iam delete-role --role-name "$ROLE_NAME" >/dev/null 2>&1 || true
  "${AWS[@]}" s3 rb "s3://$(bucket)" --force >/dev/null 2>&1 || true
  say "nuked."
}

case "${1:-help}" in
  build)  cmd_build ;;
  setup)  cmd_setup ;;
  launch) cmd_launch ;;
  status) cmd_status ;;
  fetch)  cmd_fetch ;;
  down)   cmd_down ;;
  nuke)   cmd_nuke ;;
  *) sed -n '2,30p' "$0" ;;
esac
