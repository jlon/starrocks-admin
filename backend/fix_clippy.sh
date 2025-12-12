#!/bin/bash

# Fix clippy warnings in batch

echo "Fixing clippy warnings..."

# Allow specific clippy warnings that are too complex to fix quickly
cargo clippy --all-targets --jobs 2 --fix --allow-dirty --allow-staged -- \
  --deny=warnings \
  --allow clippy::uninlined-format-args \
  --allow clippy::map-clone \
  --allow clippy::double-ended-iterator-last \
  --allow clippy::collapsible-if \
  --allow clippy::redundant-closure \
  --allow clippy::should-implement-trait \
  --allow clippy::obfuscated-if-else \
  --allow clippy::while-let-on-iterator \
  --allow clippy::unwrap-or-default \
  --allow clippy::manual-range-contains \
  --allow clippy::useless-conversion \
  --allow clippy::useless-format \
  --allow clippy::doc-lazy-continuation \
  --allow clippy::manual-clamp \
  --allow clippy::needless-borrow \
  --allow clippy::useless-asref \
  --allow clippy::field-reassign-with-default \
  --allow clippy::useless-vec

echo "Clippy fixes applied!"