# Ticket: ML Model Design

## Question

How should the taste-1 model be designed and implemented in Rust?

### Key considerations:
1. **Model architecture**: What neural network structure best captures taste preferences?
2. **Learning algorithm**: How to implement the improved RL loss function from the screenshot?
3. **Feature extraction**: What features to extract from development artifacts?
4. **Training pipeline**: How to handle incremental learning and model updates?

### Context from screenshot:
The improved RL loss function includes:
1. Advantage with Length Penalty
2. Adaptive Trust-Region Constraint (KL divergence)
3. Entropy Bonus for Exploration
4. Task-Aligned PTX (Anchored Pretraining)
5. Weight Decay toward SFT (Optional L2)

### Research needed:
- Rust ML libraries (tch-rs, linfa, burn)
- RL algorithms in Rust
- Feature engineering for code patterns
- Model serialization formats

## Type: research

## Status: open

## Assigned to: (unclaimed)