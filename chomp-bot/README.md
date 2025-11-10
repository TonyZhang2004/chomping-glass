# chomp-bot

`chomp-bot` is Tony Zhang's submission for Chomping Glass. It watches your PDA, consults a lookup table precomputed through DFS to pick a winning move when one exists, and submits the transaction. You can fire a single move on demand or let it autoplay until the game ends.

## Quick start

```bash
export PROGRAM_ID=ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D
export FEE_COLLECTOR=EGJnqcxVbhJFJ6Xnchtaw8jmPSvoLXfN2gWsY9Etz5SZ

# Compile and run unit tests
cargo test -p chomp-bot

# Inspect flags
cargo run -p chomp-bot -- --help
```

## Common commands

| Command | Description |
| --- | --- |
| `cargo run -p chomp-bot --` | Submit a single optimal move (falls back to any legal move). |
| `cargo run -p chomp-bot -- --autoplay --interval_ms 2000` | Loop forever, taking a move every 2s. |
| `cargo run -p chomp-bot -- --cash_out` | Immediately send `(0,0)` to close the PDA. |
| `cargo run -p chomp-bot -- --reset --autoplay` | Reset the PDA, wait for closure, then autoplay from a clean board. |

Important flags (see `--help` for the full list):

- `--rpc <URL>`: RPC endpoint (default `https://api.mainnet-beta.solana.com`)
- `--keypair <PATH>`: signer JSON file
- `--program` / `--collector`: override the program and fee collector pubkeys
- `--interval_ms`, `--max_moves`, `--init_if_missing`, `--last_move_wins`, `--cash_out`
- `--r` / `--c`: force a manual move in single-shot mode

## Strategy overview

The bot encodes each board as a “skyline” describing how many candies remain per row. That skyline is mapped into a 16‑bit index, which we use to address a `TABLE_SIZE = 65,536` array named `PositionTable`. Every entry is classified as:

- `Winning(row, col)`: there exists a move that forces the opponent into a losing state. The stored `(row, col)` is replayed during the game (converted back to 1-indexed coordinates).
- `Losing`: any move hands the advantage to the opponent.

When you run the CLI, it:

1. Fetches the PDA and prints the board row masks.
2. Checks whether only glass remains (`is_glass_only`).
3. Asks `pick_forced_victory` for the stored reply; if none exists, it falls back to `pick_any_legal`.
4. Builds and sends the on-chain instruction, logging the signature so you can verify the win on Solscan.

Because the lookup table is deterministic and lives in-process via `once_cell::sync::Lazy`, subsequent moves are instantaneous—no recursion or memo maps at runtime.

## Troubleshooting

- Run with `RUST_LOG=debug` to print PDA polling and move-selection details.
- Increase `--interval_ms` if your RPC endpoint throttles (`429`) during autoplay.
- If you see `No PDA found` unexpectedly, ensure your keypair has SOL to pay rent or pass `--init_if_missing=false` to stop when the account disappears.
- Anytime the on-chain layout changes, adjust `fetch_board` to match the new serialization before running the bot.

## Testing

`cargo test -p chomp-bot` verifies:

- the skyline ↔ bitmask encoding
- the forced-victory solver
- the fallback rectangle fill logic

Integrate this crate in CI by running `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test -p chomp-bot`.
