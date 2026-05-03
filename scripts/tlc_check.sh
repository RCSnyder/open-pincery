#!/usr/bin/env bash
# Run TLC on the committed TLA+ specs.
#
# Modes:
#   parse     — SANY parse + level-check only (fast, always-on).
#   simulate  — Random simulation against InvariantsV1 (default CI).
#   explore   — Bounded BFS (smaller config, for local spot-checks).
#
# Usage:
#   scripts/tlc_check.sh [parse|simulate|explore]
#
# Environment:
#   TLA_TOOLS_JAR      path to tla2tools.jar (default: ./target/tla/tla2tools.jar)
#   TLA_TOOLS_URL      where to fetch it from if missing
#                      (default: GitHub tla+ tools release).
#   TLC_SIM_NUM        number of simulation traces (default: 2000)
#   TLC_SIM_DEPTH      max depth per trace            (default: 150)
#   TLC_WORKERS        TLC worker threads             (default: 4)
#   JAVA_BIN           java executable                (default: java)
#   JAVA_XMX           JVM max heap                   (default: 2g)
set -euo pipefail

MODE="${1:-simulate}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SPEC_DIR="$ROOT/docs/input"
SPEC="OpenPinceryCanonical"
CFG_FULL="$SPEC.cfg"
CFG_SMALL="$SPEC.small.cfg"

JAR="${TLA_TOOLS_JAR:-$ROOT/target/tla/tla2tools.jar}"
JAR_URL="${TLA_TOOLS_URL:-https://github.com/tlaplus/tlaplus/releases/download/v1.8.0/tla2tools.jar}"
JAVA_BIN="${JAVA_BIN:-java}"
JAVA_XMX="${JAVA_XMX:-2g}"
TLC_SIM_NUM="${TLC_SIM_NUM:-2000}"
TLC_SIM_DEPTH="${TLC_SIM_DEPTH:-150}"
TLC_WORKERS="${TLC_WORKERS:-4}"

if [[ ! -f "$JAR" ]]; then
  echo "==> tla2tools.jar not found at $JAR, fetching..."
  mkdir -p "$(dirname "$JAR")"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$JAR" "$JAR_URL"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$JAR" "$JAR_URL"
  else
    echo "ERROR: need curl or wget to download $JAR_URL" >&2
    exit 2
  fi
fi

cd "$SPEC_DIR"

case "$MODE" in
  parse)
    echo "==> SANY parse: $SPEC.tla"
    "$JAVA_BIN" -cp "$JAR" tla2sany.SANY "$SPEC.tla"
    echo "==> SANY parse: security_append/AgenticOsSecureBehaviorV2.tla"
    "$JAVA_BIN" -cp "$JAR" tla2sany.SANY \
      security_append/AgenticOsSecureBehaviorV2.tla
    ;;

  simulate)
    echo "==> TLC simulation: $SPEC  (num=$TLC_SIM_NUM depth=$TLC_SIM_DEPTH)"
    # Use the small config for fast, repeatable CI feedback.
    "$JAVA_BIN" "-Xmx$JAVA_XMX" -XX:+UseParallelGC -cp "$JAR" tlc2.TLC \
      -config "$CFG_SMALL" \
      -workers "$TLC_WORKERS" \
      -simulate "num=$TLC_SIM_NUM" \
      -depth "$TLC_SIM_DEPTH" \
      "$SPEC"
    ;;

  explore)
    echo "==> TLC bounded BFS: $SPEC (small config)"
    "$JAVA_BIN" "-Xmx$JAVA_XMX" -XX:+UseParallelGC -cp "$JAR" tlc2.TLC \
      -config "$CFG_SMALL" \
      -workers "$TLC_WORKERS" \
      "$SPEC"
    ;;

  *)
    echo "ERROR: unknown mode '$MODE' (expected: parse|simulate|explore)" >&2
    exit 2
    ;;
esac
