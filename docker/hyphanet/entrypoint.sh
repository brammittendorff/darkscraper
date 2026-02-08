#!/bin/sh
# Custom entrypoint: start Freenet, then complete the setup wizard via FProxy API.

trapexit() {
  echo -n "Trapped SIGTERM @" >>/data/logs/term.log
  date >>/data/logs/term.log
  /fred/run.sh stop >>/data/logs/term.log
  echo "exited: $1" >>/data/logs/term.log
  exit 0
}

trap 'trapexit' SIGTERM

if [ ! -f /conf/freenet.ini ]; then
    cp /defaults/freenet.ini /conf/
    sed -i "s#ALLOWEDHOSTS#$allowedhosts#" /conf/freenet.ini
    sed -i "s#DARKNETPORT#$darknetport#" /conf/freenet.ini
    sed -i "s#OPENNETPORT#$opennetport#" /conf/freenet.ini
fi

if [ ! -f /data/seednodes.fref ]; then
    cp /fred/seednodes.fref /data/
fi

cd /fred
./run.sh console &

# Wait for FProxy to come up, then complete the setup wizard via multipart POSTs.
# The wizard uses enctype="multipart/form-data", so we must use curl -F (not -d).
# Steps: WELCOME -> BROWSER_WARNING -> DATASTORE_SIZE -> BANDWIDTH -> BANDWIDTH_MONTHLY -> COMPLETE
(
  # Wait for FProxy to be responsive (follow redirects to handle wizard redirect)
  echo "wizard-helper: waiting for FProxy..."
  for i in $(seq 1 60); do
    if curl -sfL http://127.0.0.1:8888/ -o /dev/null 2>/dev/null; then
      echo "wizard-helper: FProxy is up after ${i}s"
      break
    fi
    sleep 1
  done

  get_form_pw() {
    curl -sfL http://127.0.0.1:8888/wizard/ 2>/dev/null \
      | grep -o 'value="[A-Za-z0-9_~-]*"' \
      | head -1 \
      | sed 's/value="//;s/"//'
  }

  # Check if wizard is active (must follow redirects: / -> /wizard/)
  TITLE=$(curl -sfL http://127.0.0.1:8888/ 2>/dev/null | grep -o '<title>[^<]*</title>')
  if echo "$TITLE" | grep -qi "Set Up"; then
    echo "wizard-helper: wizard detected, completing..."
  else
    echo "wizard-helper: no wizard detected ($TITLE), skipping"
    exit 0
  fi

  FORM_PW=$(get_form_pw)
  if [ -z "$FORM_PW" ]; then
    echo "wizard-helper: ERROR - could not extract formPassword"
    exit 1
  fi

  # Step 1: WELCOME - choose low security (opennet enabled)
  curl -sf -X POST "http://127.0.0.1:8888/wizard/" \
    -F "formPassword=${FORM_PW}" \
    -F "opennet=true" \
    -F "step=WELCOME" \
    -F "incognito=false" \
    -F "presetLow=Choose low security" \
    -o /dev/null
  echo "wizard-helper: WELCOME done"
  sleep 1

  # Step 2: BROWSER_WARNING
  FORM_PW=$(get_form_pw)
  curl -sf -X POST "http://127.0.0.1:8888/wizard/" \
    -F "formPassword=${FORM_PW}" \
    -F "preset=LOW" \
    -F "opennet=true" \
    -F "step=BROWSER_WARNING" \
    -F "next=Next" \
    -o /dev/null
  echo "wizard-helper: BROWSER_WARNING done"
  sleep 1

  # Step 3: DATASTORE_SIZE (512MB)
  FORM_PW=$(get_form_pw)
  curl -sf -X POST "http://127.0.0.1:8888/wizard/" \
    -F "formPassword=${FORM_PW}" \
    -F "preset=LOW" \
    -F "opennet=true" \
    -F "step=DATASTORE_SIZE" \
    -F "ds=512M" \
    -F "next=Next" \
    -o /dev/null
  echo "wizard-helper: DATASTORE_SIZE done"
  sleep 1

  # Step 4: BANDWIDTH - accept defaults
  FORM_PW=$(get_form_pw)
  curl -sf -X POST "http://127.0.0.1:8888/wizard/" \
    -F "formPassword=${FORM_PW}" \
    -F "preset=LOW" \
    -F "opennet=true" \
    -F "step=BANDWIDTH" \
    -F "yes=Yes" \
    -o /dev/null
  echo "wizard-helper: BANDWIDTH done"
  sleep 1

  # Step 5: BANDWIDTH_MONTHLY (500GB cap)
  FORM_PW=$(get_form_pw)
  curl -sf -X POST "http://127.0.0.1:8888/wizard/" \
    -F "formPassword=${FORM_PW}" \
    -F "preset=LOW" \
    -F "opennet=true" \
    -F "step=BANDWIDTH_MONTHLY" \
    -F "capTo=500" \
    -o /dev/null
  echo "wizard-helper: BANDWIDTH_MONTHLY done"
  sleep 1

  # Verify wizard is complete
  TITLE=$(curl -sfL http://127.0.0.1:8888/ 2>/dev/null | grep -o '<title>[^<]*</title>')
  if echo "$TITLE" | grep -qi "Set Up"; then
    echo "wizard-helper: WARNING - wizard still active after completion ($TITLE)"
  else
    echo "wizard-helper: wizard completed successfully ($TITLE)"
  fi
) &

wait
