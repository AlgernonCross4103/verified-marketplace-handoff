#!/bin/sh
set -eu

: "${INFRAI_API_KEY:?set INFRAI_API_KEY first}"

cargo run -- \
  changba.com \
  chenhua@changba.com \
  changba-sellers \
  catalog-2026-09.csv \
  buyer-approved \
  order-8421
