#!/bin/bash
./target/release/substrate purge-chain --base-path /tmp/test/alice --chain local -y
./target/release/substrate \
  --base-path /tmp/test/alice \
  --chain local \
  --alice \
  --port 30333 \
  --ws-port 9945 \
  --rpc-port 9933 \
  --node-key 0000000000000000000000000000000000000000000000000000000000000001 \
  --telemetry-url "wss://telemetry.polkadot.io/submit/ 0" \
  --validator
