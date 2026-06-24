#!/bin/bash
# User-data bootstrap for a fleet spot instance. deploy.sh substitutes the
# @@PLACEHOLDERS@@ per run. Pulls the binary + run-unit from S3 and runs the unit
# under systemd (Restart=always) so it survives a process death or reboot.
set -xo pipefail
exec > /var/log/ms-bootstrap.log 2>&1

# Handy interactive tools for when you SSH in (non-fatal if unavailable).
dnf install -y htop >/dev/null 2>&1 || true

BUCKET="@@BUCKET@@"
RUN="@@RUN@@"
VARIANT="@@VARIANT@@"
LEVEL="@@LEVEL@@"
ALGO="@@ALGO@@"
XOVER="@@XOVER@@"

mkdir -p /opt/ms && cd /opt/ms
# aws CLI v2 ships on AL2023. Retry the pulls (network may still be coming up).
for i in 1 2 3 4 5 6; do
  aws s3 cp "s3://$BUCKET/$RUN/bin/morpion-solitaire" ./morpion-solitaire && break
  sleep 5
done
aws s3 cp "s3://$BUCKET/$RUN/bin/run-unit.sh" ./run-unit.sh
chmod +x morpion-solitaire run-unit.sh

cat > /etc/systemd/system/ms.service <<UNIT
[Unit]
Description=Morpion Solitaire record hunt
After=network-online.target
Wants=network-online.target
[Service]
Environment=MS_BUCKET=$BUCKET
Environment=MS_RUN_ID=$RUN
Environment=MS_VARIANT=$VARIANT
Environment=MS_LEVEL=$LEVEL
Environment=MS_ALGO=$ALGO
Environment=PERTURB_CROSSOVER=$XOVER
Environment=MS_BIN=/opt/ms/morpion-solitaire
Environment=MS_WORK=/var/lib/ms
Environment=MS_SYNC=45
Environment=NRPA_SYM=0
ExecStart=/opt/ms/run-unit.sh
Restart=always
RestartSec=3
[Install]
WantedBy=multi-user.target
UNIT

systemctl daemon-reload
systemctl enable --now ms.service
echo "BOOTSTRAP DONE"
