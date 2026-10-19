#!/usr/bin/env bash
# P10-M1-T02: generate a local TLS identity (CA + server cert/key) for the
# self-hosted Keelworks PDM stack, with SANs for the host so `oep-pdm
# --tls-cert --tls-key` terminates HTTPS trustably on U1/VM-class hosts.
#
# Usage:
#   scripts/gen-stack-certs.sh [OUT_DIR] [CN]
#     OUT_DIR  where certs land (default: ./workstreams/P10-M1/certs)
#     CN       common name / primary SAN (default: $(hostname))
#
# Produces, in OUT_DIR:
#   ca.pem        local self-signed CA (dev/on-prem root of trust)
#   cert.pem      server leaf (SAN: DNS:CN, DNS:localhost, IP:127.0.0.1)
#   key.pem       leaf private key (PKCS#8, 2048-bit RSA)
#   cert-chain.pem  = cert.pem + ca.pem (for intermediaries that need a chain)
#
# The CA key is intentionally NOT stored by default (server.key omitted); an
# operator who wants to sign extra leaves can regenerate a CA (dev) or use
# their own on-prem CA. For production, mount real PKI certs and point
# --tls-cert / --tls-key at them -- this script is the zero-config LAN path.
#
# Exit 0 iff the generated stack verifies with openssl against its own CA.
set -euo pipefail

OUT_DIR="${1:-workstreams/P10-M1/certs}"
CN="${2:-$(hostname)}"
mkdir -p "$OUT_DIR"

DAYS=825          # 27 months -- rotate before expiry (renewal is a cron/ops job)

# ---- 1. local CA (self-signed) -------------------------------------------
if [ ! -f "$OUT_DIR/ca.pem" ]; then
  openssl req -x509 -newkey rsa:2048 -sha256 -days "$DAYS" -nodes \
    -keyout "$OUT_DIR/ca.key" -out "$OUT_DIR/ca.pem" \
    -subj "/CN=Keelworks Stack Local CA (dev)" >/dev/null 2>&1
else
  echo "[certs] reuse existing ca.pem"
fi

# ---- 2. server leaf with SANs --------------------------------------------
if [ ! -f "$OUT_DIR/cert.pem" ] || [ ! -f "$OUT_DIR/key.pem" ]; then
  openssl req -newkey rsa:2048 -nodes \
    -keyout "$OUT_DIR/key.pem" -out "$OUT_DIR/csr.pem" \
    -subj "/CN=$CN" >/dev/null 2>&1
  cat > "$OUT_DIR/san.cnf" <<EOF
subjectAltName=DNS:$CN,DNS:localhost,IP:127.0.0.1
extendedKeyUsage=serverAuth
basicConstraints=CA:FALSE
keyUsage=digitalSignature,keyEncipherment
EOF
  openssl x509 -req -in "$OUT_DIR/csr.pem" -CA "$OUT_DIR/ca.pem" \
    -CAkey "$OUT_DIR/ca.key" -CAcreateserial \
    -out "$OUT_DIR/cert.pem" -days "$DAYS" -sha256 \
    -extfile "$OUT_DIR/san.cnf" >/dev/null 2>&1
  rm -f "$OUT_DIR/csr.pem" "$OUT_DIR/san.cnf"
else
  echo "[certs] reuse existing cert.pem / key.pem"
fi

# Clean serial artifacts; keep CA key only when it was just created (dev helper).
rm -f "$OUT_DIR/ca.srl"
cat "$OUT_DIR/cert.pem" "$OUT_DIR/ca.pem" > "$OUT_DIR/cert-chain.pem"

# ---- 3. verification: chain must validate + key/cert must match ----------
# Compare the leaf's public key with the key file's public key.
openssl x509 -in "$OUT_DIR/cert.pem" -noout -pubkey > "$OUT_DIR/pk.pem" 2>/dev/null
openssl pkey -in "$OUT_DIR/key.pem" -pubout > "$OUT_DIR/pk2.pem" 2>/dev/null
if openssl verify -CAfile "$OUT_DIR/ca.pem" "$OUT_DIR/cert.pem" >/dev/null 2>&1 \
   && diff -q "$OUT_DIR/pk.pem" "$OUT_DIR/pk2.pem" >/dev/null 2>&1; then
  rm -f "$OUT_DIR/pk.pem" "$OUT_DIR/pk2.pem"
  echo "[certs] verified: cert.pem chains to ca.pem"
  echo "[certs] identity : $OUT_DIR/cert.pem + $OUT_DIR/key.pem (+chain)"
  echo "[certs] to use  : oep-pdm --tls-cert $OUT_DIR/cert.pem --tls-key $OUT_DIR/key.pem"
  exit 0
else
  rm -f "$OUT_DIR/pk.pem" "$OUT_DIR/pk2.pem"
  echo "[certs] FAIL: generated stack did not verify" >&2
  exit 1
fi
