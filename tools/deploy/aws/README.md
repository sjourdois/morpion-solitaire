# AWS cold-start record-hunt fleet

`coldrun.sh` provisions a **cold-start** 5T record hunt on AWS spot: blank NRPA L4
from the empty cross, **no neural prior, no known-record seed**. The fleet's value
is the *global max* across many independent island pools (diversity), checkpointed
to S3 and safe across spot reclaims (auto-resume).

## Prerequisites

- `awscli` v2 with a working profile (SSO ok) — default `--profile dev`.
- Rust + `rustup target add aarch64-unknown-linux-gnu`.
- Debian cross linker: `sudo apt install gcc-aarch64-linux-gnu` (official apt).

## Usage

```bash
cd tools/deploy/aws
./coldrun.sh build        # cross-compile the headless aarch64 binary
./coldrun.sh setup        # S3 bucket + IAM + launch template (uploads the binary)
./coldrun.sh launch       # create the spot fleet (maintain, capacity-optimized, multi-AZ)
./coldrun.sh status       # running instances + S3 run prefixes
./coldrun.sh fetch        # sync results locally + list best games
./coldrun.sh down         # delete the fleet + terminate instances (keeps S3)
./coldrun.sh nuke         # down + delete launch template, IAM, bucket
```

Tunables (env or edit the CONFIG block):

```bash
REGION=eu-west-1 INSTANCE_TYPES="c8g.16xlarge" FLEET_SIZE=8 ./coldrun.sh launch
```

- `FLEET_SIZE` — number of spot instances (each saturates all its vCPUs).
- `INSTANCE_TYPES` — space-separated; capacity-optimized picks the cheapest
  available across them and across AZs (e.g. `"c7g.16xlarge c7g.12xlarge"`).
- `RUN_NAME` — S3 prefix + instance tag; use distinct names for parallel runs.
- `SEARCH_ARGS` — the per-instance search. Default is cold NRPA L4 `--no-symmetry`.
  A cold perturbation pool (no seed — bootstraps from the cross):
  `SEARCH_ARGS="--variant 5T search --algo perturbation --level 4 --no-symmetry --crossover 0.3" RUN_NAME=cold-pert ./coldrun.sh setup && ./coldrun.sh launch`

Shell into an instance with **SSM Session Manager** (no SSH/inbound ports):
`aws --profile dev ssm start-session --target <instance-id>`.

## Cost (estimate — re-check spot prices at launch)

Spot prices drift; figures are eu-west-3, ~2026-06. The cost is ~100 % compute.

| Fleet | vCPU | Duration | Approx total |
|---|---|---|---|
| 4× c7g.16xlarge | 256 | 1 week | **$320–600** |
| 8× c7g.16xlarge | 512 | 2 weeks | **$1,300–2,400** |
| 16× c7g.16xlarge | 1024 | 2 weeks | **$2,600–4,800** |

S3 + egress < $2 (checkpoints are KB); EBS root ≈ $0.64/instance/month. The fleet
is `maintain` — **it bills until `down`**. Set a calendar reminder; there is no
auto-stop.

## Reality check

Cold-pure (no accelerator) caps ~131 on a single L4 run; a large diverse fleet
pushes the global max higher, realistically ~140–150. 178 is a lottery ticket. The
value is a clean from-nothing benchmark, not a guaranteed record.
