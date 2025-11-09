# Technical Write-Up

## Architecture Overview
The bot is split into three small crates modules: `config` owns CLI/env surfaces, `solana` wraps all RPC + PDA instructions, and `game` exposes pure move-generation utilities. This separation keeps networking, key-management, and solver logic decoupled, letting us unit-test the solver without touching Solana and swap out transport pieces (e.g., using a prioritized RPC) without rewriting game logic. The `main` loop orchestrates configuration, memoized solver state, and the execution mode (single move vs. autoplay), which keeps stateful concerns localized and traceable in logs.

## Most Challenging Technical Aspects
Interfacing a local machine with the on-chain program was the trickiest part: we must derive the PDA deterministically, fetch its serialized board state, and mutate it via a move instruction, all while dealing with RPC rate limits and eventual consistency. The client needs to keep signing keys safe locally, yet stream moves fast enough to keep up with on-chain opponents. Error handling is nuanced—missing PDAs, partially confirmed cash-outs, or serialization mismatches can all cause desync, so the code polls for closure after resets and double-checks fetched data shapes before trusting a board snapshot.

## Problem-Solving Approach
1. Recreate the minimal Solana interaction surface (PDA derivation, board fetch, transaction send) and validate it against the live program.
2. Build a deterministic board representation that matches the on-chain format, then implement pure solver logic over it.
3. Layer configuration (CLI/env) and control flow (single move vs. autoplay) on top, keeping logging and memoization optional but well-scoped.
4. Iterate with end-to-end dry runs against a dev cluster to verify transaction lifecycles and solver choices.

## Strategy Overview
The solver performs a DFS with memoization over the 5x8 Chomp board, flagging “winning” boards and returning the first move that leads the opponent into a losing state; autoplay falls back to any legal move when the solver cannot find a forced win (e.g., when the board is already poisoned). Because opponents—including AI agents—may deviate from optimal play or mirror our strategy, we cannot guarantee victory against a perfectly optimal adversary; the bot merely maximizes the chance of forcing a loss when such a state exists.

## Strengthening the Strategy
- Integrate opponent modeling to detect non-optimal patterns and bias the solver toward lines that exploit those tendencies.
- Add Monte Carlo or minimax rollouts with heuristic pruning to explore deeper future states when memoized data is sparse.

## AI Disclosure
I collaborated with Codex (powered by GPT-5) to implement my original project sketch, refine the Solana integration, and debug RPC/solver edge cases. I guided the work, monitored code generation and documentation under assistance with Codex.
