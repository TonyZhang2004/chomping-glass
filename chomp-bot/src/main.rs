use anyhow::{Context, Result};
use env_logger::Env;
use log::{info, warn};

mod config;
mod solana;
mod game;

use crate::config::Cli;
use crate::solana::{fetch_board, get_game_pda, reset_game_pda, send_move};
use crate::game::{pick_any_legal, pick_forced_victory, is_glass_only};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::{read_keypair_file, Keypair}, signer::Signer};
use std::{thread, time::Duration};
use clap::Parser;

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    info!("starting chomp-strat-bot; autoplay={}, single-move={}", cli.autoplay, !cli.autoplay);

    let program_id: Pubkey = cli.program_id.parse().context("Invalid PROGRAM_ID pubkey")?;
    let fee_collector: Pubkey = cli.fee_collector.parse().context("Invalid FEE_COLLECTOR pubkey")?;
    let payer_path = config::expand_home(&cli.keypair_path);
let payer: Keypair = read_keypair_file(&payer_path)
    .map_err(|e| anyhow::anyhow!("failed to read keypair at {}: {}", payer_path, e))?;
    let rpc = RpcClient::new(cli.rpc_url.clone());
    let (game_pda, _bump) = get_game_pda(&program_id, &payer.pubkey());

    if cli.reset {
        reset_game_pda(&rpc, &program_id, &fee_collector, &payer, &game_pda)?;
    }

    if cli.autoplay {
        run_autoplay(&rpc, &program_id, &fee_collector, &payer, &game_pda, &cli)?;
    } else {
        run_single_move(&rpc, &program_id, &fee_collector, &payer, &game_pda, &cli)?;
    }
    Ok(())
}

fn run_autoplay(
    rpc: &RpcClient,
    program_id: &Pubkey,
    fee_collector: &Pubkey,
    payer: &Keypair,
    game_pda: &Pubkey,
    cli: &Cli,
) -> Result<()> {
    info!(
        "Autoplay ON (interval={}ms, max_moves={}, last_move_wins={}, reset={}, init_if_missing={})",
        cli.interval_ms, cli.max_moves, cli.last_move_wins, cli.reset, cli.init_if_missing
    );

    let mut moves_sent = 0u32;
    loop {
        match fetch_board(rpc, game_pda)? {
            Some(board) => {
                print_board("board", &board);
                if is_glass_only(board) {
                    info!("Only glass remains — game over.");
                    break;
                }

                let (r, c) = pick_forced_victory(board)
                    .or_else(|| pick_any_legal(board))
                    .unwrap_or((0, 0));
                info!("chosen: ({},{})", r, c);
                if r == 0 && c == 0 {
                    info!("No safe move — stopping.");
                    break;
                }

                send_move(rpc, program_id, fee_collector, payer, game_pda, r, c)?;
                moves_sent += 1;
                if moves_sent >= cli.max_moves {
                    warn!("Reached max_moves={} — stopping.", cli.max_moves);
                    break;
                }
                thread::sleep(Duration::from_millis(cli.interval_ms));
            }
            None => {
                if !cli.init_if_missing {
                    warn!("PDA missing — stopping autoplay");
                    break;
                }
                info!("No PDA found — starting a NEW game by making the first move.");
                let empty = [0u8; 5];
                let (r, c) = pick_forced_victory(empty)
                    .or_else(|| pick_any_legal(empty))
                    .unwrap_or((5, 1));
                info!("opening: ({},{})", r, c);
                send_move(rpc, program_id, fee_collector, payer, game_pda, r, c)?;
                thread::sleep(Duration::from_millis(cli.interval_ms));
            }
        }
    }

    if let Some(final_board) = fetch_board(rpc, game_pda)? {
        print_board("final", &final_board);
    } else {
        info!("final board: account missing/closed");
    }
    Ok(())
}

fn run_single_move(
    rpc: &RpcClient,
    program_id: &Pubkey,
    fee_collector: &Pubkey,
    payer: &Keypair,
    game_pda: &Pubkey,
    cli: &Cli,
) -> Result<()> {
    match fetch_board(rpc, game_pda)? {
        Some(board) => {
            print_board("current", &board);
            if is_glass_only(board) {
                info!("Only glass remains — game ended.");
                return Ok(());
            }

            let (r, c) = if cli.cash_out {
                (0, 0)
            } else if let (Some(r), Some(c)) = (cli.row, cli.col) {
                (r, c)
            } else {
                pick_forced_victory(board)
                    .or_else(|| pick_any_legal(board))
                    .unwrap_or((0, 0))
            };

            info!("chosen move: ({},{})", r, c);
            if r == 0 && c == 0 {
                info!("No safe move / cash-out.");
                return Ok(());
            }

            send_move(rpc, program_id, fee_collector, payer, game_pda, r, c)?;
            if let Some(updated) = fetch_board(rpc, game_pda)? {
                print_board("updated", &updated);
            } else {
                warn!("account closed after our move");
            }
        }
        None => {
            if !cli.init_if_missing {
                warn!("game account missing/closed — aborting");
                return Ok(());
            }
            info!("No PDA found — starting NEW game.");
            let empty = [0u8; 5];
            let (r, c) = pick_forced_victory(empty)
                .or_else(|| pick_any_legal(empty))
                .unwrap_or((5, 1));
            info!("opening: ({},{})", r, c);
            send_move(rpc, program_id, fee_collector, payer, game_pda, r, c)?;
            if let Some(updated) = fetch_board(rpc, game_pda)? {
                print_board("new board", &updated);
            }
        }
    }
    Ok(())
}

fn print_board(tag: &str, s: &[u8; 5]) {
    info!("{}:", tag);
    for (i, row) in s.iter().enumerate() {
        println!("row{}: {:08b}", i + 1, row);
    }
}
