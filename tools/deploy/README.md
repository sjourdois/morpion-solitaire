# `tools/deploy/` — AWS spot fleet for record hunting

Launch a fleet of cheap **spot** instances (AWS Graviton) that each run an
independent Morpion Solitaire search and stream their best game to S3, so the
global best accumulates across the fleet. The bet is **diversity × compute**: many
independent island pools running for a long time maximise the chance of a
record-breaking tail result.

Everything runs on the plain **`dev`** AWS profile — `iam:PassRole` on
`ms-fleet-role` was granted to it, so no admin profile is needed at run time.

---

## TL;DR

```bash
# 1. build the aarch64 binary once (see "Build the binary")
# 2. launch a fleet
tools/deploy/deploy.sh --region eu-west-1 --run-id record --capacity 16 --level 4
# 3. watch it (global best + ssh commands)
tools/deploy/monitor.sh --run-id record
# 4. stop + clean up
tools/deploy/teardown.sh --run-id record
```

---

## The scripts

| Script | What it does | How it's run |
|---|---|---|
| **`deploy.sh`** | Uploads the binary + `run-unit.sh` to S3, builds a launch template, and creates an EC2 Fleet (`type=maintain`) of spot instances spread over AZs and instance types. The fleet keeps the target capacity alive, auto-replacing interrupted spot instances. | **you**, with `--options` |
| **`monitor.sh`** | Reads S3 + EC2: fleet status, the running instances (with a ready-to-paste `ssh` command each), every instance's best score, and the **global best** (saved to `tools/deploy/global-best-<run>.msr`). | **you**, `--run-id …` |
| **`teardown.sh`** | Deletes the fleet (terminating its instances), the launch template, and the security group. **Keeps** the S3 results, the bucket, and the IAM role/profile for reuse. | **you**, `--run-id …` |
| **`run-unit.sh`** | The per-instance worker. Runs **one open-ended search** that self-bounds its RAM (`--max-memory 75%`) and writes `best.msr` + checkpoint + `progress.log` into a run dir, uploading them to S3 frequently and on a spot-interruption notice. A thin supervisor only restarts (resuming from the checkpoint) if the search crashes. | **the instance** (systemd), not by hand |
| **`bootstrap.sh`** | EC2 *user-data*. `deploy.sh` substitutes its `@@PLACEHOLDERS@@` per run; on boot it pulls the binary + `run-unit.sh` from S3 and runs the unit under systemd (`Restart=always`). | **the instance**, at boot |
| **`admindev.sh`** | Thin **audited** wrapper around `aws` for the rare privileged (admin-profile) one-off operations — appends every call to `admindev-audit.log`. Not used by the fleet itself. | rarely, for setup |

---

## How a run works

- **One run = one `--run-id`** (also the S3 prefix and the EC2 tag). Several runs can
  coexist in the same account/bucket.
- **Each instance is independent** — its own island pool (diversity), keyed in S3 by
  instance-id. There is no coordination; the global best is just the max over all
  instances, computed by `monitor.sh`.
- **S3 layout:**
  ```
  s3://<bucket>/<run>/bin/        morpion-solitaire, run-unit.sh   (uploaded by deploy.sh)
  s3://<bucket>/<run>/inst/<iid>/ best.msr, search-checkpoint-*.msc,
                                  progress.log, heartbeat.txt,
                                  INTERRUPTED.txt, run.log
  ```
- **Memory is bounded in the binary** (`--max-memory`), not by chunking/ulimit/OOM —
  an NRPA island restarts from a fresh policy before exceeding its share.
- **Spot interruptions:** the unit watches the 2-minute notice and does a final S3
  upload (+ an `INTERRUPTED.txt` marker); the fleet then launches a replacement,
  which resumes the hunt fresh. The best is already durable in S3, so nothing is lost.
- **Crash recovery:** the unit checkpoints every 60 s; a crashed search restarts and
  resumes from the checkpoint (a corrupt checkpoint is detected and dropped).

---

## `deploy.sh` options

```
--region   REGION   AWS region                          (eu-west-1)
--run-id   ID        run name / S3 prefix / tag           (run1)
--capacity N         spot instances to maintain           (4)
--variant  V         5T|5D|4T|4D                           (5T)
--level    L         NRPA level (nrpa only)               (4)
--algo     A         nrpa|perturbation                     (nrpa)
--xover    R         crossover rate (perturbation only)    (0)
--types    "t1 t2"  instance types to mix                 (c7g.xlarge c8g.xlarge)
--key      NAME      SSH key pair (must exist in region)   (smj)
--profile  NAME      AWS profile                           (dev)
--bucket   NAME      S3 bucket               (ms-recordhunt-<account>)
--ami      ID         AMI                     (latest AL2023 arm64)
--bin      PATH      aarch64 binary to upload
```

`deploy.sh` is idempotent for a given `--run-id`: re-running it refreshes the launch
template and grows/keeps the fleet. It writes `tools/deploy/.state-<run>` for
`monitor.sh`/`teardown.sh` (git-ignored).

---

## Build the binary

A headless, statically-current aarch64 (Graviton) binary — no GUI deps:

```bash
rustup target add aarch64-unknown-linux-gnu        # + apt: gcc-aarch64-linux-gnu
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
  cargo build --release -p morpion-solitaire --no-default-features \
    --target aarch64-unknown-linux-gnu
```

It needs only `GLIBC_2.34` (= AL2023's glibc), so it runs as-is on the fleet. (A
fully static `--target aarch64-unknown-linux-musl` build also works if you need to
run on an older distro.)

---

## One-time setup (fresh account)

The fleet expects these to exist (created once; `teardown.sh` keeps them):

```bash
# S3 bucket
aws --profile dev --region eu-west-1 s3api create-bucket \
  --bucket "ms-recordhunt-$(aws --profile dev sts get-caller-identity --query Account --output text)" \
  --create-bucket-configuration LocationConstraint=eu-west-1

# IAM role + instance profile (S3-scoped) — needs an admin profile (see admindev.sh)
aws iam create-role --role-name ms-fleet-role --assume-role-policy-document \
  '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}'
aws iam put-role-policy --role-name ms-fleet-role --policy-name ms-s3 --policy-document \
  '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["s3:GetObject","s3:PutObject","s3:ListBucket"],"Resource":["arn:aws:s3:::ms-recordhunt-<ACCT>","arn:aws:s3:::ms-recordhunt-<ACCT>/*"]}]}'
aws iam create-instance-profile --instance-profile-name ms-fleet-profile
aws iam add-role-to-instance-profile --instance-profile-name ms-fleet-profile --role-name ms-fleet-role

# Let the dev profile pass that role at launch (scoped, ec2 only). For an SSO `dev`
# this is an inline policy on its permission set (done from the management account):
#   iam:PassRole on arn:aws:iam::<ACCT>:role/ms-fleet-role, condition
#   iam:PassedToService = ec2.amazonaws.com

# SSH key in the region (import your existing public key)
aws --profile dev --region eu-west-1 ec2 import-key-pair \
  --key-name smj --public-key-material fileb://smj.pub
```

---

## Notes

- **Region:** default `eu-west-1`. `c8g` (Graviton4, ~+27% over `c7g`) is **not** in
  `eu-west-3`; use eu-west-1 / eu-central-1 / a us region for it.
- **Cost (spot, eu-west-1):** `c8g.xlarge` ≈ `$0.067/h`. e.g. 16 instances ≈ `$1/h`
  (~`$180`/week). `teardown.sh` stops the spend.
- **`admindev-audit.log`** is a local record of privileged actions (git-ignored).
