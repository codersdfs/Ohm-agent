# Wayfinder Map: Taste System for Omega Agent

## Destination

Build a comprehensive taste system for the Omega Agent that learns developer preferences from all development artifacts (code patterns, workflow, tools) using a Rust-based ML model, with hybrid local/cloud storage, integrated into the CLI for code generation, review, and workflow optimization.

## Notes

- **Domain**: AI coding assistant / developer tooling
- **Existing infrastructure**: Omega Agent already has a `taste` component in the Mechanized Gate (harness crate)
- **Key reference**: Command Code's taste system (TASTE_SUMMARY.md) - uses meta neuro-symbolic AI model `taste-1`
- **ML approach**: The screenshot shows an improved RL loss function with 5 components (advantage, trust-region, entropy, PTX, weight decay)
- **Tech stack**: Rust for performance, hybrid local/cloud storage

## Decisions so far

<!-- Initially empty - will be populated as tickets are resolved -->

## Not yet specified

- How to integrate with existing Omega infrastructure (harness, memory, providers)
- Cloud storage backend and authentication mechanism
- Real-time learning pipeline architecture
- Model serialization and deployment strategy

## Out of scope

<!-- Initially empty - scope boundaries will be defined as planning progresses -->