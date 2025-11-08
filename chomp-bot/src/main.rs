use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::{thread, time::Duration};

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    system_program,
    transaction::Transaction,
};

const PROGRAM_ID: &str = "ChompZg47TcVy5fk2LxPEpW6SytFYBES5SHoqgrm8A4D";
const FEE_COLLECTOR: &str = "EGJnqcxVbhJFJ6Xnchtaw8jmPSvoLXfN2gWsY9Etz5SZ";

// Bitmasks, matching on-chain constants M (single-bit) and B (left-filled)
const M: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];
const B: [u8; 8] = [0x80, 0xC0, 0xE0, 0xF0, 0xF8, 0xFC, 0xFE, 0xFF];

fn main() -> Result<(), Box<dyn Error>> {
    // ---- tiny arg parser (no clap) ----
    let mut rpc_url = "https://api.mainnet-beta.solana.com".to_string();
    let mut keypair_path = default_keypair_path()?;
    let mut r_arg: Option<u8> = None;
    let mut c_arg: Option<u8> = None;
    let mut cash_out = false;

    // autoplay controls
    let mut autoplay = false;
    let mut interval_ms: u64 = 1500; // wait between moves to let state finalize
    let mut max_moves: u32 = 200;    // safety cap

    // winner rule (poison): last move loses by default
    let mut last_move_wins: bool = false;

    // reset + init controls
    let mut do_reset = false;
    let mut init_if_missing = true; // if PDA missing, make first move to initialize

    let mut it = env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--rpc" => if let Some(v) = it.next() { rpc_url = v; }
            "--keypair" => if let Some(v) = it.next() { keypair_path = v; }
            "--r" => if let Some(v) = it.next() { r_arg = v.parse().ok(); }
            "--c" => if let Some(v) = it.next() { c_arg = v.parse().ok(); }
            "--cash_out" => cash_out = true,
            "--auto" | "--autoplay" => autoplay = true,
            "--interval_ms" => if let Some(v) = it.next() { interval_ms = v.parse().unwrap_or(1500); }
            "--max_moves" => if let Some(v) = it.next() { max_moves = v.parse().unwrap_or(200); }
            "--last_move_wins" => if let Some(v) = it.next() {
                let vv = v.to_ascii_lowercase();
                last_move_wins = !(vv == "false" || vv == "0" || vv == "no");
            },
            "--reset" => do_reset = true,
            "--init_if_missing" => if let Some(v) = it.next() {
                let vv = v.to_ascii_lowercase();
                init_if_missing = !(vv == "false" || vv == "0" || vv == "no");
            },
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            _ => {}
        }
    }

    let program_id: Pubkey = PROGRAM_ID.parse()?;
    let fee_collector: Pubkey = FEE_COLLECTOR.parse()?;
    let payer: Keypair = read_keypair_file(expand_home(&keypair_path))?;
    let rpc = RpcClient::new(rpc_url);

    // PDA seed = [player_pubkey] exactly like on-chain program
    let (game_pda, _bump) = Pubkey::find_program_address(&[payer.pubkey().as_ref()], &program_id);

    // If asked, reset: send (0,0) to close PDA (if it exists), then wait until closed.
    if do_reset {
        reset_game_pda(&rpc, &program_id, &fee_collector, &payer, &game_pda)?;
    }

    // Memo table for the solver (shared across all moves this run)
    let mut memo: HashMap<[u8; 5], (bool, Option<(u8, u8)>)> = HashMap::new();

    if autoplay {
        eprintln!(
            "Autoplay ON (interval={}ms, max_moves={}, last_move_wins={}, reset={}, init_if_missing={})",
            interval_ms, max_moves, last_move_wins, do_reset, init_if_missing
        );
        let mut moves_sent: u32 = 0;
        let mut last_moved_us: Option<bool> = None;

        loop {
            match fetch_board(&rpc, &game_pda)? {
                Some(board) => {
                    if is_terminal_poison(board) {
                        // Only glass left: current player is forced to eat glass ⇒ current player loses.
                        let winner = decide_winner(false, last_move_wins);
                        eprintln!("Only glass remains — game ended. Winner: {}", winner);
                        break;
                    }

                    eprintln!("Board:");
                    for i in 0..5 {
                        eprintln!("{:08b}", board[i]);
                    }

                    let (r, c) = choose_move_solver(board, &mut memo)
                        .unwrap_or_else(|| choose_any_safe(board).unwrap_or((0, 0)));
                    eprintln!("Chosen: ({},{})", r, c);

                    if r == 0 && c == 0 {
                        let winner = decide_winner(false, last_move_wins);
                        eprintln!("No safe move (only glass remains). Winner: {}", winner);
                        break;
                    }

                    send_move(&rpc, &program_id, &fee_collector, &payer, &game_pda, r, c)?;

                    last_moved_us = Some(true);
                    moves_sent += 1;
                    if moves_sent >= max_moves {
                        let winner = decide_winner(true, last_move_wins);
                        eprintln!("Reached max_moves={} — stopping. Winner (by rule): {}", max_moves, winner);
                        break;
                    }

                    thread::sleep(Duration::from_millis(interval_ms));

                    match fetch_board(&rpc, &game_pda)? {
                        Some(b2) => {
                            if is_terminal_poison(b2) {
                                let winner = decide_winner(true, last_move_wins);
                                eprintln!("Only glass remained after our move. Winner: {}", winner);
                                break;
                            }
                        }
                        None => {
                            let winner = decide_winner(true, last_move_wins);
                            eprintln!("Account closed after our move. Winner: {}", winner);
                            break;
                        }
                    }
                }
                None => {
                    // PDA missing: optionally initialize by making the first move from an empty board
                    if !init_if_missing {
                        let winner = decide_winner(last_moved_us.unwrap_or(false), last_move_wins);
                        eprintln!("Game account missing/closed — stopping autoplay. Winner: {}", winner);
                        break;
                    }

                    eprintln!("No PDA found — starting a NEW game by making the first move.");
                    let empty = [0u8; 5];
                    let (r, c) = choose_move_solver(empty, &mut memo)
                        .or_else(|| choose_any_safe(empty))
                        .unwrap_or((5, 1)); // safe default
                    eprintln!("Opening move: ({},{})", r, c);

                    send_move(&rpc, &program_id, &fee_collector, &payer, &game_pda, r, c)?;
                    last_moved_us = Some(true);

                    thread::sleep(Duration::from_millis(interval_ms));
                    // Continue loop; next iteration will see a real board.
                }
            }
        }

        // Show final board (if still exists)
        match fetch_board(&rpc, &game_pda)? {
            Some(final_board) => {
                eprintln!("Final board:");
                for i in 0..5 {
                    eprintln!("{:08b}", final_board[i]);
                }
            }
            None => eprintln!("Final board: account missing/closed."),
        }
        return Ok(());
    }

    // -------- single-move path (original behavior) --------
    match fetch_board(&rpc, &game_pda)? {
        Some(board) => {
            eprintln!("Current board:");
            for i in 0..5 {
                eprintln!("{:08b}", board[i]);
            }

            if is_terminal_poison(board) {
                let winner = decide_winner(false, last_move_wins);
                eprintln!("Only glass remains — game ended. Winner: {}", winner);
                return Ok(());
            }

            let (r, c) = if cash_out {
                (0u8, 0u8)
            } else if let (Some(r), Some(c)) = (r_arg, c_arg) {
                (r, c)
            } else {
                choose_move_solver(board, &mut memo)
                    .unwrap_or_else(|| choose_any_safe(board).unwrap_or((0, 0)))
            };
            eprintln!("Chosen move: ({},{})", r, c);

            if r == 0 && c == 0 {
                let winner = decide_winner(false, last_move_wins);
                eprintln!("No safe move / cash-out. Winner: {}", winner);
                return Ok(());
            }

            send_move(&rpc, &program_id, &fee_collector, &payer, &game_pda, r, c)?;

            match fetch_board(&rpc, &game_pda)? {
                Some(updated) => {
                    eprintln!("Updated board:");
                    for i in 0..5 {
                        eprintln!("{:08b}", updated[i]);
                    }
                    if is_terminal_poison(updated) {
                        let winner = decide_winner(true, last_move_wins);
                        eprintln!("Only glass remains after our move. Winner: {}", winner);
                    }
                }
                None => {
                    let winner = decide_winner(true, last_move_wins);
                    eprintln!("Account closed after our move. Winner: {}", winner);
                }
            }
        }
        None => {
            // Single-move path with missing PDA: if allowed, start first move; else exit.
            if !init_if_missing {
                eprintln!("Game account missing/closed or not initialized — aborting.");
                return Ok(());
            }
            eprintln!("No PDA found — starting a NEW game by making the first move.");
            let empty = [0u8; 5];
            let (r, c) = choose_move_solver(empty, &mut memo)
                .or_else(|| choose_any_safe(empty))
                .unwrap_or((5, 1));
            eprintln!("Opening move: ({},{})", r, c);

            send_move(&rpc, &program_id, &fee_collector, &payer, &game_pda, r, c)?;

            match fetch_board(&rpc, &game_pda)? {
                Some(updated) => {
                    eprintln!("New board after opening move:");
                    for i in 0..5 {
                        eprintln!("{:08b}", updated[i]);
                    }
                }
                None => eprintln!("Account still missing after opening move."),
            }
        }
    }

    Ok(())
}

// ---------- CLI help & small utils ----------

fn print_usage() {
    println!("Usage: chomp-bot [--rpc URL] [--keypair PATH] [--r N --c M] [--cash_out] [--auto|--autoplay] [--interval_ms N] [--max_moves N] [--last_move_wins BOOL] [--reset] [--init_if_missing BOOL]");
    println!("  --rpc URL             RPC endpoint (default mainnet-beta)");
    println!("  --keypair PATH        Path to keypair (default ~/.config/solana/id.json)");
    println!("  --r, --c              Move coordinates (1<=r<=5, 1<=c<=8). If omitted, bot picks.");
    println!("  --cash_out            Send (0,0) to close PDA after you’ve already played once.");
    println!("  --auto|--autoplay     Keep making moves until done (no manual input).");
    println!("  --interval_ms N       Milliseconds to sleep between moves (default 1500).");
    println!("  --max_moves N         Safety cap on number of moves (default 200).");
    println!("  --last_move_wins B    true/false (default false = poison rule).");
    println!("  --reset               Close existing PDA (if any) before playing (fresh start).");
    println!("  --init_if_missing B   true/false (default true). If PDA missing, make first move to initialize.");
}

fn default_keypair_path() -> Result<String, Box<dyn Error>> {
    let home = std::env::var("HOME")?;
    Ok(format!("{home}/.config/solana/id.json"))
}

fn expand_home(p: &str) -> String {
    if p.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{}", &p[2..]);
        }
    }
    p.to_string()
}

// ---------- On-chain account helpers ----------

fn fetch_board(rpc: &RpcClient, game_pda: &Pubkey) -> Result<Option<[u8; 5]>, Box<dyn Error>> {
    match rpc.get_account(game_pda) {
        Ok(acc) if acc.data.len() >= 5 => {
            let mut s = [0u8; 5];
            s.copy_from_slice(&acc.data[..5]);
            Ok(Some(s))
        }
        _ => Ok(None),
    }
}

fn send_move(
    rpc: &RpcClient,
    program_id: &Pubkey,
    fee_collector: &Pubkey,
    payer: &Keypair,
    game_pda: &Pubkey,
    r: u8,
    c: u8,
) -> Result<(), Box<dyn Error>> {
    let ix = make_move_ix(program_id, &payer.pubkey(), game_pda, fee_collector, r, c)?;
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], blockhash);
    let sig = rpc.send_and_confirm_transaction(&tx)?;
    println!("✅ Sent move ({},{}): {}", r, c, sig);
    Ok(())
}

fn make_move_ix(
    program_id: &Pubkey,
    player: &Pubkey,
    game_pda: &Pubkey,
    fee_collector: &Pubkey,
    r: u8,
    c: u8,
) -> Result<Instruction, Box<dyn Error>> {
    if !(r == 0 && c == 0) {
        if !(1 <= r && r <= 5 && 1 <= c && c <= 8) {
            return Err("r in 1..=5 and c in 1..=8 (or (0,0) to cash out)".into());
        }
    }
    let data = [(r << 4) | c];
    Ok(Instruction {
        program_id: *program_id,
        data: data.to_vec(),
        accounts: vec![
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(*player, true),
            AccountMeta::new(*game_pda, false),
            AccountMeta::new(*fee_collector, false),
        ],
    })
}

// ---------- Reset flow (via cash-out) ----------

fn reset_game_pda(
    rpc: &RpcClient,
    program_id: &Pubkey,
    fee_collector: &Pubkey,
    payer: &Keypair,
    game_pda: &Pubkey,
) -> Result<(), Box<dyn Error>> {
    eprintln!("Reset requested: checking current game PDA...");
    let exists = fetch_board(rpc, game_pda)?.is_some();

    if !exists {
        eprintln!("No existing PDA — already fresh.");
        return Ok(());
    }

    eprintln!("Closing PDA by sending cash-out (0,0)...");
    let ix = make_move_ix(program_id, &payer.pubkey(), game_pda, fee_collector, 0, 0)?;
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], blockhash);
    let sig = rpc.send_and_confirm_transaction(&tx)?;
    println!("✅ Cash-out tx: {}", sig);

    // Wait until the account is closed / missing
    for i in 0..20 {
        thread::sleep(Duration::from_millis(500));
        if fetch_board(rpc, game_pda)?.is_none() {
            eprintln!("PDA closed ({} checks). Fresh start ready.", i + 1);
            return Ok(());
        }
    }
    eprintln!("Warning: PDA still present after waiting — continuing anyway.");
    Ok(())
}

// ---------- Game-specific logic ----------

// In "Chomping Glass", the glass is at bottom-right: row=5, col=8.
// Terminal (pre-loss) position: ONLY that glass remains: rows 1..4 = 0xFF, row5 = 0xFE.
fn is_terminal_poison(s: [u8; 5]) -> bool {
    s[0] == 0xFF && s[1] == 0xFF && s[2] == 0xFF && s[3] == 0xFF && s[4] == 0xFE
}

// Decide winner given who moved last and the rule.
fn decide_winner(last_moved_us: bool, last_move_wins: bool) -> &'static str {
    let we_win = if last_move_wins { last_moved_us } else { !last_moved_us };
    if we_win { "You" } else { "Opponent" }
}

// Whether a cell (r,c) is currently empty (legal to chomp).
fn is_legal(s: [u8; 5], r: u8, c: u8) -> bool {
    (s[(r - 1) as usize] & M[(c - 1) as usize]) == 0
}

// Apply a move to produce the next state: set left-filled bits B[c-1] on rows r..=5.
fn apply_move(mut s: [u8; 5], r: u8, c: u8) -> [u8; 5] {
    let mask = B[(c - 1) as usize];
    for rr in (r - 1) as usize..5 {
        s[rr] |= mask;
    }
    s
}

// Any safe (non-glass) move, preferring deeper rows & leftmost columns.
fn choose_any_safe(s: [u8; 5]) -> Option<(u8, u8)> {
    for r in (1..=5u8).rev() {
        for c in 1..=7u8 {
            if is_legal(s, r, c) {
                return Some((r, c));
            }
        }
    }
    None
}

// -------- Perfect solver (memoized) for poison rule --------
//
// A position is WIN for the player to move iff there exists a SAFE move (c<=7) to
// a position that is a LOSS for the opponent. If ONLY glass remains (terminal-poison),
// current player loses. Any move with c=8 (eating glass) is immediate loss for mover,
// so the solver never chooses c=8.
//
// Returns: Some((r,c)) winning move if the position is winning; None if losing.
fn choose_move_solver(
    s: [u8; 5],
    memo: &mut HashMap<[u8; 5], (bool, Option<(u8, u8)>)>,
) -> Option<(u8, u8)> {
    if let Some(&(win, mv)) = memo.get(&s) {
        return if win { mv } else { None };
    }

    // Terminal (pre-loss): only glass remains
    if is_terminal_poison(s) {
        memo.insert(s, (false, None));
        return None;
    }

    // Try all SAFE moves (avoid c=8 which would eat glass)
    for r in (1..=5u8).rev() {
        for c in 1..=7u8 {
            if !is_legal(s, r, c) {
                continue;
            }
            let t = apply_move(s, r, c);

            // Opponent's best reply
            let opp_mv = choose_move_solver(t, memo);
            // If opponent has NO winning move (their position is losing),
            // then (r,c) is a winning move for us.
            if opp_mv.is_none() {
                memo.insert(s, (true, Some((r, c))));
                return Some((r, c));
            }
        }
    }

    // No winning move found → losing position
    memo.insert(s, (false, None));
    None
}
