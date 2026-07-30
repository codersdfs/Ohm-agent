# Ticket: Data Collection Pipeline

## Question

How should the taste system collect and process development artifacts for learning?

### Key considerations:
1. **Artifact types**: What specific data to collect from each artifact type?
2. **Collection mechanism**: Real-time hooks vs batch processing?
3. **Feature extraction**: How to convert raw artifacts into learnable features?
4. **Privacy considerations**: What data should/shouldn't be collected?

### Artifact categories:
- **Code structure**: File organization, module boundaries, API patterns, naming
- **Development workflow**: Git commits, PRs, reviews, testing, CI/CD
- **Tool choices**: Libraries, build tools, linters, formatters, configs
- **Real-time actions**: Accept/reject decisions, edits, communications

### Research needed:
- Git history analysis techniques
- Code structure analysis (AST parsing)
- Real-time event collection methods
- Data normalization and feature engineering

## Type: research

## Status: open

## Assigned to: (unclaimed)