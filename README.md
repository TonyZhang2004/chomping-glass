# Chomping Glass

Chomping Glass is a Solana take on the “poisoned chocolate bar” puzzle. Players alternate chomping rectangles out of a 5 × 8 board; everything up and to the left of the chosen square is removed, and whoever is forced to eat the poisonous bottom‑right square loses. This repo contains:

- the on-chain Rust program (`src/`)
- a React front-end (`chomping-glass/`)
- an autoplay CLI solver (`chomp-bot/`)

![Game board](./images/game.png)

## Program deployment

The program is immutably deployed to mainnet-beta at:

```
ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D
```

You can independently verify the binary with `solana-verify`:

```bash
solana-verify verify-from-repo -um \
  --program-id ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D \
  https://github.com/jarry-xiao/chomping-glass
```

## Running the dApp

```bash
cd chomping-glass
yarn install
yarn start
```

The UI connects to your wallet, submits signed moves on-chain, streams PDA updates, and highlights both your moves and the AI’s responses. Confirming a move automatically eats every square above and to the left of the chosen cell. Use the “Give Up” button to cash out with the `(0,0)` instruction if you want to reset mid-game.

## CLI autoplay bot

The `chomp-bot` crate plays optimally (within compute limits) by consulting a precomputed table of solved board states. Key commands:

```bash
# Inspect usage
cargo run -p chomp-bot -- --help

# Fire a single optimal move
cargo run -p chomp-bot --

# Continuous autoplay with a 2s interval
cargo run -p chomp-bot -- --autoplay --interval_ms 2000

# Run unit tests for the solver
cargo test -p chomp-bot
```

Set `PROGRAM_ID` and `FEE_COLLECTOR` via env vars or flags, and ensure your keypair has enough SOL to cover rent + compute. The bot prints the board in binary, the move it chose, and the resulting transaction signature so you can track wins on Solscan.
