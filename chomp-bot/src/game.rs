use once_cell::sync::Lazy;

const ROW_COUNT: usize = 5;
const COL_COUNT: usize = 8;
const ROWS_U8: u8 = ROW_COUNT as u8;
const COLS_U8: u8 = COL_COUNT as u8;
const POISON_ROW: u8 = ROWS_U8;
const POISON_COL: u8 = COLS_U8;
const TABLE_SIZE: usize = 1 << 16;
const BIT_TEST: [u8; COL_COUNT] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

static STRATEGY: Lazy<PositionTable> = Lazy::new(PositionTable::new);

pub fn is_glass_only(board: [u8; ROW_COUNT]) -> bool {
    board.iter().take(ROW_COUNT - 1).all(|row| *row == 0xFF) && board[ROW_COUNT - 1] == 0xFE
}

fn move_is_open(board: [u8; ROW_COUNT], r: u8, c: u8) -> bool {
    board[(r - 1) as usize] & BIT_TEST[(c - 1) as usize] == 0
}

pub fn pick_any_legal(board: [u8; ROW_COUNT]) -> Option<(u8, u8)> {
    for r in (1..=ROWS_U8).rev() {
        for c in 1..=COLS_U8 {
            if (r, c) == (POISON_ROW, POISON_COL) {
                continue;
            }
            if move_is_open(board, r, c) {
                return Some((r, c));
            }
        }
    }
    move_is_open(board, POISON_ROW, POISON_COL).then_some((POISON_ROW, POISON_COL))
}

pub fn pick_forced_victory(board: [u8; ROW_COUNT]) -> Option<(u8, u8)> {
    STRATEGY
        .best_reply(&bitmask_to_skyline(board))
        .map(|(row, col)| ((row as u8) + 1, col as u8))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Classified {
    Unexplored,
    Winning(u8, u8),
    Losing,
}

/// Tracks how many squares are already eaten from each row.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Skyline(pub [u8; ROW_COUNT]);

impl Skyline {
    pub fn encode(&self) -> usize {
        let mut idx = 0usize;
        let mut trailing = COL_COUNT as u8;
        self.0.iter().for_each(|&val| {
            if trailing > val {
                idx <<= (trailing - val) as usize;
                trailing = val;
            }
            idx = (idx << 1) | 1;
        });
        idx << trailing as usize
    }

    pub fn decode(mut encoded: usize) -> Self {
        let mut rows = [0u8; ROW_COUNT];
        rows[ROW_COUNT - 1] = encoded.trailing_zeros() as u8;
        encoded >>= (rows[ROW_COUNT - 1] + 1) as usize;

        let mut zeros_seen = 0u8;
        let mut cursor = ROW_COUNT - 1;

        while encoded != 0 {
            if encoded & 1 == 1 {
                cursor -= 1;
                rows[cursor] = rows[cursor + 1] + zeros_seen;
                zeros_seen = 0;
                encoded >>= 1;
            } else {
                zeros_seen += 1;
                encoded >>= 1;
            }
        }

        Self(rows)
    }
}

pub struct PositionTable {
    book: [Classified; TABLE_SIZE],
}

impl PositionTable {
    pub fn new() -> Self {
        let mut book = [Classified::Unexplored; TABLE_SIZE];
        // Base cases: completely eaten and glass-only endings.
        book[0b1111100000000] = Classified::Winning(0xFF, 0xFF);
        book[0b1111010000000] = Classified::Losing;

        fn dfs(idx: usize, book: &mut [Classified]) {
            if !matches!(book[idx], Classified::Unexplored) {
                return;
            }

            let snapshot = Skyline::decode(idx);
            let mut found_response = false;

            for r in 0..ROW_COUNT as u8 {
                let current = snapshot.0[r as usize];
                for c in (current + 1)..=COLS_U8 {
                    let mut next = snapshot;
                    for fill_row in 0..=r {
                        let slot = fill_row as usize;
                        next.0[slot] = next.0[slot].max(c);
                    }
                    let next_idx = next.encode();
                    if book[next_idx] == Classified::Unexplored {
                        dfs(next_idx, book);
                    }
                    if book[next_idx] == Classified::Losing {
                        book[idx] = Classified::Winning(r, c);
                        found_response = true;
                    }
                }
            }

            if !found_response {
                book[idx] = Classified::Losing;
            }
        }

        dfs(0b11111, &mut book);

        Self { book }
    }

    pub fn best_reply(&self, skyline: &Skyline) -> Option<(usize, usize)> {
        match self.book[skyline.encode()] {
            Classified::Winning(0xFF, 0xFF) => None,
            Classified::Winning(r, c) => Some((r as usize, c as usize)),
            _ => None,
        }
    }
}

fn bitmask_to_skyline(board: [u8; ROW_COUNT]) -> Skyline {
    let mut rows = [0u8; ROW_COUNT];
    for (i, &mask) in board.iter().enumerate() {
        rows[i] = mask.leading_ones() as u8;
    }
    Skyline(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_board_has_safe_move() {
        let mv = pick_forced_victory([0u8; ROW_COUNT]);
        assert!(mv.is_some());
    }

    #[test]
    fn terminal_is_losing() {
        let s = [0xFF, 0xFF, 0xFF, 0xFF, 0xFE];
        assert!(pick_forced_victory(s).is_none());
    }

    #[test]
    fn solver_prefers_last_column_when_only_option() {
        let board = [0xFE; ROW_COUNT];
        let mv = pick_forced_victory(board).expect("move");
        assert_eq!(mv.1, 8);
    }

    #[test]
    fn skyline_round_trip_cases() {
        let cases = [
            [8, 8, 8, 8, 8],
            [8, 8, 8, 8, 7],
            [8, 8, 8, 8, 0],
            [8, 8, 8, 0, 0],
            [0, 0, 0, 0, 0],
            [4, 3, 2, 1, 0],
            [8, 6, 4, 2, 0],
        ];
        for case in cases {
            let skyline = Skyline(case);
            assert_eq!(Skyline::decode(skyline.encode()), skyline);
        }
    }
}
