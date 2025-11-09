# chomp-bot

A production-ready command-line bot that plays Solana “Chomp/Glass” by monitoring the per-player PDA, computing an optimal (or at least safe) move, and submitting the transaction on your behalf. The tool can either fire a single move on demand or run indefinitely in autoplay mode with optional PDA reset/re-init flows.

## Prerequisites
- **Rust** 1.74+ with `cargo` (install via [rustup](https://rustup.rs/)).
- **Solana toolchain** that matches the on-chain program this client talks to. The crate pins `solana-client`/`solana-sdk` to `1.14.12`, so install `solana-cli 1.14.12` if you need to inspect accounts or manage keypairs locally.
- **Keypair** that will pay fees and own the PDA (default: `~/.config/solana/id.json`).
- **RPC endpoint** with write access to the cluster you want to play on (mainnet-beta by default).

## Setup
```bash
# From the repo root
cargo build -p chomp-bot --release

# Verify everything compiles and solver tests pass
cargo test -p chomp-bot
```

Set the program and fee collector once in your shell profile (or export them before running):
```bash
export PROGRAM_ID=ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D
export FEE_COLLECTOR=EGJnqcxVbhJFJ6Xnchtaw8jmPSvoLXfN2gWsY9Etz5SZ
```

## Configuration
All runtime settings are exposed as CLI flags (and some have env fallbacks). Run `cargo run -p chomp-bot -- --help` for the full list. The most important switches:

| Flag / Env             | Purpose                                                                 | Default                                  |
|------------------------|-------------------------------------------------------------------------|------------------------------------------|
| `--rpc <URL>`          | Solana RPC endpoint                                                     | `https://api.mainnet-beta.solana.com`    |
| `--keypair <PATH>`     | Fee payer/player keypair                                                | `~/.config/solana/id.json`               |
| `--program <PUBKEY>`   | Game program ID (`PROGRAM_ID` env)                                      | `ChompZg47…`                             |
| `--collector <PUBKEY>` | Fee collector account (`FEE_COLLECTOR` env)                             | `EGJnqcxV…`                              |
| `--autoplay`           | Loop forever, submitting moves on interval                              | `false`                                  |
| `--interval_ms <u64>`  | Sleep between autoplay moves                                            | `1500`                                   |
| `--max_moves <u32>`    | Stop autoplay after N moves                                             | `200`                                    |
| `--init_if_missing`    | Create a PDA automatically if one is missing                            | `true`                                   |
| `--reset`              | Cash-out (0,0) to close the PDA before starting                         | `false`                                  |
| `--r/--c <u8>`         | Force a manual move (row/column) in single-move mode                    | `None`                                   |
| `--cash_out`           | Send `(0,0)` immediately to exit the game                               | `false`                                  |
| `RUST_LOG=<level>`     | Log verbosity (handled by `env_logger`)                                 | `info`                                   |

The solver memoizes explored boards in-memory, so long-running autoplay sessions should be started as a single process to benefit from the cache.

## Running

### Single move
```bash
cargo run -p chomp-bot -- \
  --rpc https://api.mainnet-beta.solana.com \
  --keypair ~/.config/solana/id.json \
  --program $PROGRAM_ID \
  --collector $FEE_COLLECTOR
```
- If `--r`/`--c` are omitted, the solver selects a safe move.  
- Include `--cash_out` to send `(0,0)` and close the PDA.  
- Use `--init_if_missing=false` if you only want to act when a board already exists.

### Autoplay
```bash
cargo run -p chomp-bot -- \
  --autoplay \
  --interval_ms 2000 \
  --max_moves 500 \
  --rpc https://api.mainnet-beta.solana.com
```
The loop reads the PDA, prints a bitwise board view, chooses a move (falling back to any legal move if needed), and submits the transaction. It stops when:
- The board reaches the terminal “glass only” state.
- No safe move exists.
- `--max_moves` is hit.
- The PDA disappears and `--init_if_missing=false`.

### Resetting the PDA
Passing `--reset` issues a `(0,0)` “cash-out” move, waits for the PDA to close, and only then proceeds. This is useful when testing new strategies from a known empty board.

## Troubleshooting
- Ensure your keypair has enough SOL to cover compute units and the PDA rent.  
- RPC providers may throttle frequent writes; increase `--interval_ms` if you see `429`/`rate limit` errors.  
- Enable debug logs with `RUST_LOG=debug` to trace solver decisions and PDA polling.  
- If the PDA data layout changes on-chain, update `fetch_board` to match the new serialization before running against that program.

## Testing & CI
`cargo test -p chomp-bot` exercises the recursive solver and ensures the terminal board is handled correctly. Integrate the crate into your CI by running `cargo fmt --check`, `cargo clippy -- -D warnings`, and the tests mentioned above to guard future changes.
