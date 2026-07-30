# Taste System Plan Summary

## What We're Building

A comprehensive taste system for the Omega Agent that learns developer preferences from all development artifacts, using a Rust-based ML model with hybrid local/cloud storage.

## Current State

The Omega Agent already has a basic taste implementation in `src-tauri/crates/harness/src/taste.rs` that performs static rule-based checks:
- Rust: Excessive `.clone()`, `.unwrap()`, `String` error types, commented code
- TypeScript: `any` type, `var` usage, non-null assertions, magic numbers
- Python: Bare `except:`, mutable default arguments

This is **static analysis** - it doesn't learn from developer behavior over time.

## What We Want to Build

A **dynamic learning system** inspired by Command Code's taste feature:
- **Meta neuro-symbolic AI model** (`taste-1`) that learns from interactions
- **Continuous reinforcement learning** from accept/reject decisions
- **Hybrid local/cloud storage** for preferences
- **Integration with CLI** for code generation, review, and workflow optimization

## Key Technical Decisions Needed

### 1. Architecture Design
- How to integrate with existing Omega infrastructure
- Module boundaries and API surface
- Data flow from collection to learning to application

### 2. ML Model Design
- Neural network architecture for taste preferences
- Implementation of improved RL loss function (from screenshot):
  - Advantage with Length Penalty
  - Adaptive Trust-Region Constraint
  - Entropy Bonus for Exploration
  - Task-Aligned PTX (Anchored Pretraining)
  - Weight Decay toward SFT
- Feature extraction from development artifacts

### 3. Data Collection Pipeline
- What artifacts to collect (code, workflow, tools, real-time actions)
- Collection mechanisms (real-time hooks vs batch processing)
- Feature engineering and normalization

### 4. Storage System Design
- Local storage format and structure
- Cloud backend options
- Sync mechanisms and conflict resolution

### 5. CLI Integration
- New CLI commands for taste management
- Integration with Plan/Build/Review agents
- Enhanced Gate rule enforcement

### 6. Cloud Sync Mechanism
- Authentication and authorization
- Sync protocols
- Team sharing features

## Wayfinder Plan Structure

### Map Location
`C:\Users\pwong\Ohm-agent\wayfinder\map.md`

### Tickets Location
`C:\Users\pwong\Ohm-agent\wayfinder\tickets\`

### Tracker
`C:\Users\pwong\Ohm-agent\wayfinder\tracker.json`

## Next Steps

1. **Start with Architecture Design** - Understand integration points with existing Omega crates
2. **Run Research Subagents** - Gather information on each ticket topic
3. **Resolve Tickets One at a Time** - Make decisions and record them
4. **Update Map** - Graduate fog items into new tickets as they become specifiable
5. **Hand Off to Implementation** - When way is clear, begin building

## Research Areas

### For Architecture Design
- Current Omega crate structure and dependencies
- Memory system (hermes) architecture
- Provider abstraction patterns
- Pipeline state machine structure

### For ML Model Design
- Rust ML libraries (tch-rs, linfa, burn)
- RL algorithms in Rust
- Feature engineering for code patterns
- Model serialization formats

### For Data Collection
- Git history analysis techniques
- Code structure analysis (AST parsing)
- Real-time event collection methods
- Privacy considerations

### For Storage System
- Existing Omega memory system (SQLite + FTS5)
- Cloud storage options
- Sync protocols and conflict resolution
- Serialization formats

### For CLI Integration
- Current CLI command structure
- Agent prompt injection mechanisms
- Gate rule configuration
- User preference management

### For Cloud Sync
- Authentication methods
- Sync protocols
- Cloud backend options
- Team management features