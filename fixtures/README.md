# Synthetic acceptance inputs

The exact source tables and expected values are preserved in [Northstar scenarios](../design/forma-northstar-demo/SCENARIOS.md). APP-011 materializes CSV/XLSX inputs, malformed specimens, two organizations and named principals, and verifies their arithmetic independently.

Northstar: 1,140,000 total bytes; 900,000 unmatched; 114,000 bytes/minute over the full ten-minute half-open window. Independent reuse: 210,000 bytes; 21,000 bytes/minute. Spend Review: 64,000 cents total; 45,000-cent threshold queue; integer USD, no implicit conversion.

Keep fixtures synthetic and deterministic. A fixture or seeded definition cannot stand in for actual user creation when qualifying the two-app demo. No runtime fixtures have been executed at this preparation checkpoint.
