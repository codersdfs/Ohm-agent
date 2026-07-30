# Wayfinder Plan: Taste System for Omega Agent

## What I've Done

I've created a Wayfinder plan for building a comprehensive taste system for the Omega Agent. The plan uses decision tickets to map out the key decisions that need to be made before implementation can begin.

### Files Created

1. **`wayfinder/map.md`** - The main map with destination, notes, and sections for decisions, fog, and out-of-scope items
2. **`wayfinder/tracker.json`** - JSON file tracking ticket status, assignments, and blocking relationships
3. **`wayfinder/README.md`** - Instructions on how to use the wayfinder plan
4. **`wayfinder/PLAN_SUMMARY.md`** - Detailed summary of what we're building and key decisions needed
5. **`wayfinder/tickets/001-architecture-design.md`** - Ticket for architecture design decisions
6. **`wayfinder/tickets/002-ml-model-design.md`** - Ticket for ML model design decisions
7. **`wayfinder/tickets/003-data-collection.md`** - Ticket for data collection pipeline decisions
8. **`wayfinder/tickets/004-storage-system.md`** - Ticket for storage system design decisions
9. **`wayfinder/tickets/005-cli-integration.md`** - Ticket for CLI integration decisions
10. **`wayfinder/tickets/006-cloud-sync.md`** - Ticket for cloud sync mechanism decisions

## Current State of Omega Agent

### Existing Taste Implementation
The Omega Agent already has a basic taste implementation in `src-tauri/crates/harness/src/taste.rs` that performs static rule-based checks:
- **Rust**: Excessive `.clone()`, `.unwrap()`, `String` error types, commented code
- **TypeScript**: `any` type, `var` usage, non-null assertions, magic numbers
- **Python**: Bare `except:`, mutable default arguments

This is **static analysis** - it doesn't learn from developer behavior over time.

### What We Want to Build
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

## How to Use the Wayfinder Plan

### View the Map
Open `wayfinder/map.md` to see the overall destination and current status.

### View Tickets
Open `wayfinder/tickets/` directory to see all decision tickets. Each ticket contains:
- A specific question or decision to resolve
- Context and research needed
- Current status and assignment

### Work Through the Map
To resolve a ticket:
1. **Claim it**: Assign yourself to the ticket
2. **Research**: Use `/research` subagent to gather information
3. **Decide**: Make the decision based on research
4. **Record**: Post resolution as a comment and close the ticket
5. **Update Map**: Add decision to "Decisions so far" section

### Track Progress
The `wayfinder/tracker.json` file tracks:
- Ticket status (open/in-progress/closed)
- Assignments
- Blocking relationships

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