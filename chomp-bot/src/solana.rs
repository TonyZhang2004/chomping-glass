use anyhow::{bail, Context, Result};
use log::{info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program, transaction::Transaction,
};
use std::{thread, time::Duration};

pub fn get_game_pda(program_id: &Pubkey, player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[player.as_ref()], program_id)
}

pub fn fetch_board(rpc: &RpcClient, game_pda: &Pubkey) -> Result<Option<[u8; 5]>> {
    match rpc.get_account(game_pda) {
        Ok(acc) if acc.data.len() >= 5 => {
            let mut s = [0u8; 5];
            s.copy_from_slice(&acc.data[..5]);
            Ok(Some(s))
        }
        Ok(_) => Ok(None),
        Err(_) => Ok(None),
    }
}

pub fn send_move(
    rpc: &RpcClient,
    program_id: &Pubkey,
    fee_collector: &Pubkey,
    payer: &Keypair,
    game_pda: &Pubkey,
    r: u8,
    c: u8,
) -> Result<()> {
    let ix = make_move_ix(program_id, &payer.pubkey(), game_pda, fee_collector, r, c)?;
    let bh = rpc.get_latest_blockhash().context("fetch blockhash")?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], bh);
    let sig = rpc.send_and_confirm_transaction(&tx).context("send tx")?;
    info!("✅ Sent move ({},{}): {}", r, c, sig);
    Ok(())
}

pub fn reset_game_pda(
    rpc: &RpcClient,
    program_id: &Pubkey,
    fee_collector: &Pubkey,
    payer: &Keypair,
    game_pda: &Pubkey,
) -> Result<()> {
    info!("reset requested: checking current game PDA...");
    let exists = fetch_board(rpc, game_pda)?.is_some();
    if !exists {
        info!("No existing PDA — already fresh.");
        return Ok(());
    }

    info!("Closing PDA by sending cash-out (0,0)...");
    let ix = make_move_ix(program_id, &payer.pubkey(), game_pda, fee_collector, 0, 0)?;
    let bh = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], bh);
    let sig = rpc.send_and_confirm_transaction(&tx)?;
    info!("✅ Cash-out tx: {}", sig);

    for i in 0..20 {
        thread::sleep(Duration::from_millis(500));
        if fetch_board(rpc, game_pda)?.is_none() {
            info!("PDA closed ({} checks). Fresh start ready.", i + 1);
            return Ok(());
        }
    }
    warn!("PDA still present after waiting — continuing anyway.");
    Ok(())
}

fn make_move_ix(
    program_id: &Pubkey,
    player: &Pubkey,
    game_pda: &Pubkey,
    fee_collector: &Pubkey,
    r: u8,
    c: u8,
) -> Result<Instruction> {
    if !(r == 0 && c == 0) {
        if !(1 <= r && r <= 5 && 1 <= c && c <= 8) {
            bail!("r in 1..=5 and c in 1..=8 (or (0,0) to cash out)");
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
