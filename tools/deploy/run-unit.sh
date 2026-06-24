#!/usr/bin/env bash
# ============================================================================
# run-unit.sh — runs on each spot instance of the Morpion Solitaire record fleet.
#
# Runs ONE open-ended search that self-bounds its memory (--max-memory) and writes
# its best game + checkpoint + progress log into a run dir (--run-dir); uploads them
# to S3 frequently and on a spot-interruption notice, so the global best is never
# lost. A thin supervisor restarts (resuming from the checkpoint) only if the search
# crashes. Independent per instance: its own island pool (diversity), keyed in S3 by
# instance-id.
#
# Robustness (validated on a real instance):
#   - IMDSv2 token for every metadata call (AL2023 is IMDSv2-only).
#   - set -u, NOT -e: errors are handled, never kill the unit.
#   - S3 uploads retry; a failed upload never stops the search.
#   - Spot 2-min notice -> final upload -> clean stop (replacement resumes fresh).
#   - Memory is bounded IN the binary (--max-memory) — no chunking/ulimit/OOM-killer.
#
# Env (from the bootstrap user-data): MS_BUCKET, MS_RUN_ID, and optionally
#   MS_VARIANT (5T), MS_LEVEL (4), MS_ALGO (nrpa), MS_SYNC (90s), MS_MAXMEM
#   (default 75% of RAM), NRPA_SYM (0), PERTURB_CROSSOVER, MS_BIN, MS_WORK.
# ============================================================================
set -uo pipefail

: "${MS_BUCKET:?MS_BUCKET required}"
: "${MS_RUN_ID:?MS_RUN_ID required}"
MS_VARIANT="${MS_VARIANT:-5T}"
MS_LEVEL="${MS_LEVEL:-4}"
MS_ALGO="${MS_ALGO:-nrpa}"
MS_SYNC="${MS_SYNC:-90}"
# Memory budget defaults to 75% of total RAM (leaves room for the OS + uploads). The
# binary itself resolves the `%` against total RAM and restarts islands before
# exceeding it, so the instance never OOMs.
MS_MAXMEM="${MS_MAXMEM:-75%}"
export NRPA_SYM="${NRPA_SYM:-0}"
MS_BIN="${MS_BIN:-./morpion-solitaire}"

WORK="${MS_WORK:-/var/lib/ms}"
mkdir -p "$WORK"
# All search outputs (best.msr, checkpoint, progress.log) land in $WORK via --run-dir
# — explicit, uploadable paths, with no HOME/XDG manipulation.
BEST="$WORK/best.msr"
LOG="$WORK/run.log"
STOP="$WORK/.stop"
rm -f "$STOP"

# --- IMDSv2 metadata helpers ------------------------------------------------
imds() { # imds <path> ; echoes value or empty (never fails the script)
  local tok
  tok=$(curl -sS -m 3 -X PUT "http://169.254.169.254/latest/api/token" \
        -H "X-aws-ec2-metadata-token-ttl-seconds: 60" 2>/dev/null) || return 0
  curl -sS -m 3 -H "X-aws-ec2-metadata-token: $tok" \
       "http://169.254.169.254/latest/meta-data/$1" 2>/dev/null || true
}
IID="$(imds instance-id)"; IID="${IID:-$(hostname)}"
AZ="$(imds placement/availability-zone)"
S3="s3://$MS_BUCKET/$MS_RUN_ID/inst/$IID"

log() { echo "$(date -Is) [$IID] $*" | tee -a "$LOG"; }

# --- durable upload (best + checkpoint + progress), with retry --------------
upload() {
  local f
  for f in "$BEST" "$WORK"/search-checkpoint-*.msc "$WORK/progress.log"; do
    [ -f "$f" ] || continue
    local try
    for try in 1 2 3; do
      aws s3 cp "$f" "$S3/$(basename "$f")" --only-show-errors && break
      sleep $((try * 2))
    done
  done
  # a small heartbeat so the monitor can see liveness + AZ
  printf '%s\t%s\t%s\n' "$(date -Is)" "$AZ" "$IID" \
    | aws s3 cp - "$S3/heartbeat.txt" --only-show-errors 2>/dev/null || true
}

# --- spot-interruption watcher: final upload then signal stop ---------------
# The spot/instance-action endpoint returns HTTP 404 (with a body!) when there is
# NO interruption, and 200 with the action JSON when there is. So we MUST key on
# the 200 status, not on a non-empty body — else the 404 body is a false positive.
spot_interrupting() {
  local tok code
  tok=$(curl -sS -m 3 -X PUT "http://169.254.169.254/latest/api/token" \
        -H "X-aws-ec2-metadata-token-ttl-seconds: 60" 2>/dev/null) || return 1
  code=$(curl -s -m 3 -o /dev/null -w '%{http_code}' \
         -H "X-aws-ec2-metadata-token: $tok" \
         "http://169.254.169.254/latest/meta-data/spot/instance-action" 2>/dev/null)
  [ "$code" = "200" ]
}
watch_spot() {
  while [ ! -f "$STOP" ]; do
    if spot_interrupting; then
      log "SPOT INTERRUPTION NOTICE — final upload, stopping"
      upload
      # Verifiable proof in S3 (we can't SSH): an interruption marker + the log.
      printf '%s\t%s\t%s\n' "$(date -Is)" "$AZ" "$IID" \
        | aws s3 cp - "$S3/INTERRUPTED.txt" --only-show-errors 2>/dev/null || true
      aws s3 cp "$LOG" "$S3/run.log" --only-show-errors 2>/dev/null || true
      touch "$STOP"
      pkill -f "morpion-solitaire search" 2>/dev/null || true
      return
    fi
    sleep 5
  done
}

# --- periodic uploader ------------------------------------------------------
periodic() {
  while [ ! -f "$STOP" ]; do
    sleep "$MS_SYNC"
    upload
  done
}

log "start: algo=$MS_ALGO variant=$MS_VARIANT level=$MS_LEVEL az=$AZ maxmem=$MS_MAXMEM bucket=$MS_BUCKET run=$MS_RUN_ID"
"$MS_BIN" --version >>"$LOG" 2>&1 || log "WARN: binary --version failed"

watch_spot &  WSPID=$!
periodic   &  PPID_=$!

# --- supervisor: ONE open-ended, self-bounded search; restart only on crash --
# The binary runs open-ended and caps its own memory (--max-memory), writes a fresh
# best.msr as it improves, and checkpoints periodically. We only re-enter the loop
# if it actually exits (a crash) — resuming from the checkpoint.
if [ "$MS_ALGO" = "perturbation" ]; then
  ALGO_ARGS=(--algo perturbation)
else
  ALGO_ARGS=(--algo nrpa --level "$MS_LEVEL")
fi
while [ ! -f "$STOP" ]; do
  CK=$(ls "$WORK"/search-checkpoint-*.msc 2>/dev/null | head -1)
  start=$(date +%s)
  if [ -n "$CK" ]; then
    log "resume search (from $(basename "$CK"))"
    "$MS_BIN" search --resume "$CK" --run-dir "$WORK" --max-memory "$MS_MAXMEM" \
      --checkpoint-interval 60s --time 720h --target-score 999 >>"$LOG" 2>&1
  else
    log "cold search ($MS_ALGO $MS_VARIANT xover=${PERTURB_CROSSOVER:-0})"
    "$MS_BIN" search "${ALGO_ARGS[@]}" --variant "$MS_VARIANT" --run-dir "$WORK" \
      --max-memory "$MS_MAXMEM" --checkpoint-interval 60s --time 720h \
      --target-score 999 >>"$LOG" 2>&1
  fi
  rc=$?; dur=$(( $(date +%s) - start ))
  log "search exited rc=$rc dur=${dur}s"
  # A resume that dies almost instantly means a bad/corrupt checkpoint (e.g. killed
  # mid-write by an interruption) — drop it so the restart starts cold instead of
  # fast-looping forever on the broken file.
  if [ -n "$CK" ] && [ "$dur" -lt 10 ]; then
    log "resume failed fast — discarding checkpoint"
    rm -f "$WORK"/search-checkpoint-*.msc
  fi
  upload
  [ -f "$STOP" ] && break
  sleep 3
done

upload
kill "$WSPID" "$PPID_" 2>/dev/null || true
log "unit stopped"
