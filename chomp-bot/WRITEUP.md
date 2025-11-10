# Technical Write-Up

## Architecture Overview
The crate is organized around three modules:

- `config`: parses CLI flags + env vars (powered by `clap`) and normalizes defaults.
- `solana`: derives the PDA, fetches the 5-byte board state, submits signed instructions, and exposes helpers for “cash out”/reset flows.
- `game`: contains the pure strategy engine (`PositionTable`, `Skyline`, `pick_forced_victory`, etc.).

`main.rs` glues everything together by configuring logging, loading the player keypair, then dispatching to either `run_single_move` or `run_autoplay`. This separation lets us unit-test the solver without touching RPC, while keeping all side effects (network + signing) inside the `solana` module.

## Technical Challenges

1. **Interacting with the on-chain program reliably** – All Solana calls live in `solana.rs`, which derives the PDA (`Pubkey::find_program_address`), fetches the five-byte board (`rpc.get_account`), and builds the single-byte instruction `[(row << 4) | col]`. Cash-outs reuse the same instruction with `(0,0)` and the client polls the PDA every 500 ms until it disappears so we never reuse a stale account. Every transaction signature is logged, giving us a Solscan trail and quick feedback if the chain rejects a move. Exposing `--interval_ms`, `--max_moves`, and `--init_if_missing` lets operators tune write pressure and decide whether to bootstrap a new PDA automatically.
2. **Figuring out a deterministic lookup-table strategy** – Rather than run DFS + memoization every move, we encode each board into a “skyline” and pack it into a 16-bit index. `PositionTable::new()` performs a single DFS over all 5 × 8 boards at startup, labeling each index as `Winning(row, col)` or `Losing`. At runtime `pick_forced_victory` simply replays the stored coordinates while `pick_any_legal` handles losing states. This turns move selection into an O(1) table lookup, survives process restarts, and guarantees the same response for a given board.

## Strategy Details

1. Encode each 5 × 8 board as a monotonic “skyline”: how many candies remain per row from left to right.
2. Map that skyline to a compact 16-bit index (the `Skyline::encode` bit-packing step).
3. During build, lazily populate `PositionTable.book` by DFS’ing the entire game tree once. Each state is marked as:
   - `Classified::Winning(r, c)` if there exists a move forcing the opponent into a Losing state.
   - `Classified::Losing` when every move hands the advantage to the opponent.
4. At runtime, `pick_forced_victory` simply looks up the encoded skyline and replays the stored `(row, col)` (converted back to human 1-indexed coordinates). When no forced win exists, `pick_any_legal` returns the first legal square, prioritizing non-poison moves.

Because the glass sits in the bottom-right corner and moves eat upward/leftward, this perspective flip lets the solver reuse the same bitmasks the program stores on-chain—no translation layer needed.

## Operating the Bot

- **Single move:** `cargo run -p chomp-bot -- --rpc <URL> --program <ID> --collector <FEE>`
- **Autoplay:** `cargo run -p chomp-bot -- --autoplay --interval_ms 2000`
- **Reset then autoplay:** `cargo run -p chomp-bot -- --reset --autoplay`

Every invocation prints the latest board (binary rows), the move coordinates, and the resulting transaction signature so you can verify it on Solscan. If only the poisonous square remains, the loop stops with “Only glass remains — game over.”

## AI Disclosure

I collaborated with Codex (powered by GPT‑5) for code implementation, refactors, and documentation polish. All Solana credentials, testing, and deployment decisions were performed manually. I reviewed every change, supplied strategy requirements, and validated that the final implementation matched my design.
