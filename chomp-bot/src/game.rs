use std::collections::HashMap;

const M: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];
const B: [u8; 8] = [0x80, 0xC0, 0xE0, 0xF0, 0xF8, 0xFC, 0xFE, 0xFF];

pub fn is_terminal_poison(s: [u8; 5]) -> bool {
    s[0] == 0xFF && s[1] == 0xFF && s[2] == 0xFF && s[3] == 0xFF && s[4] == 0xFE
}

pub fn is_legal(s: [u8; 5], r: u8, c: u8) -> bool {
    (s[(r - 1) as usize] & M[(c - 1) as usize]) == 0
}

pub fn apply_move(mut s: [u8; 5], r: u8, c: u8) -> [u8; 5] {
    let mask = B[(c - 1) as usize];
    for rr in (r - 1) as usize..5 {
        s[rr] |= mask;
    }
    s
}

pub fn choose_any_safe(s: [u8; 5]) -> Option<(u8, u8)> {
    for r in (1..=5u8).rev() {
        for c in 1..=7u8 {
            if is_legal(s, r, c) {
                return Some((r, c));
            }
        }
    }
    None
}

pub fn choose_move_solver(
    s: [u8; 5],
    memo: &mut HashMap<[u8; 5], (bool, Option<(u8, u8)>)>,
) -> Option<(u8, u8)> {
    if let Some(&(win, mv)) = memo.get(&s) {
        return if win { mv } else { None };
    }
    if is_terminal_poison(s) {
        memo.insert(s, (false, None));
        return None;
    }

    for r in (1..=5u8).rev() {
        for c in 1..=7u8 {
            if !is_legal(s, r, c) {
                continue;
            }
            let t = apply_move(s, r, c);
            if choose_move_solver(t, memo).is_none() {
                memo.insert(s, (true, Some((r, c))));
                return Some((r, c));
            }
        }
    }

    memo.insert(s, (false, None));
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_board_has_safe_move() {
        let mut memo = HashMap::new();
        let mv = choose_move_solver([0u8; 5], &mut memo);
        assert!(mv.is_some());
    }

    #[test]
    fn terminal_is_losing() {
        let s = [0xFF, 0xFF, 0xFF, 0xFF, 0xFE];
        let mut memo = HashMap::new();
        assert!(choose_move_solver(s, &mut memo).is_none());
    }
}
