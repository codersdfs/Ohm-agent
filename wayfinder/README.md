# Wayfinder Plan: Taste System for Omega Agent

## Overview

This directory contains a Wayfinder plan for building a comprehensive taste system for the Omega Agent. The plan uses decision tickets to map out the key decisions that need to be made before implementation can begin.

## How to Use

### View the Map
Open `map.md` to see the overall destination and current status.

### View Tickets
Open `tickets/` directory to see all decision tickets. Each ticket contains:
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
The `tracker.json` file tracks:
- Ticket status (open/in-progress/closed)
- Assignments
- Blocking relationships

## Current Status

### Open Tickets (6)
1. **Architecture Design** - How to structure the taste system within Omega
2. **ML Model Design** - How to implement the taste-1 model in Rust
3. **Data Collection Pipeline** - How to collect and process development artifacts
4. **Storage System Design** - Hybrid local/cloud storage architecture
5. **CLI Integration** - How to integrate with Omega CLI commands
6. **Cloud Sync Mechanism** - Cloud synchronization for team sharing

### Frontier (Takeable Now)
All tickets are currently unblocked and can be worked on in parallel.

## Destination

Build a comprehensive taste system for the Omega Agent that learns developer preferences from all development artifacts (code patterns, workflow, tools) using a Rust-based ML model, with hybrid local/cloud storage, integrated into the CLI for code generation, review, and workflow optimization.

## Next Steps

1. Start with **Architecture Design** ticket to understand integration points
2. Run research subagents on all tickets in parallel
3. Resolve tickets one at a time, updating the map as decisions are made
4. Graduate fog items into new tickets as they become specifiable