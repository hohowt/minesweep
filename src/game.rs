use rand::rng;
use rand::seq::SliceRandom;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Expert,
}

impl Difficulty {
    pub fn config(&self) -> (u32, u32, u32) {
        match self {
            Difficulty::Beginner => (9, 9, 10),
            Difficulty::Intermediate => (16, 16, 40),
            Difficulty::Expert => (16, 30, 99),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellContent {
    Empty,
    Mine,
    Number(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellState {
    Hidden,
    Revealed,
    Flagged,
    QuestionMark,
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub content: CellContent,
    pub state: CellState,
    pub exploded: bool,   // For red background on lost
    pub wrong_flag: bool, // For crossed out mine on lost
}

impl Cell {
    pub fn new() -> Self {
        Self {
            content: CellContent::Empty,
            state: CellState::Hidden,
            exploded: false,
            wrong_flag: false,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum GameStatus {
    NotStarted,
    Playing,
    Won,
    Lost,
}

pub struct Minesweeper {
    pub rows: u32,
    pub cols: u32,
    pub mines: u32,
    pub cells: Vec<Cell>,
    pub status: GameStatus,
    pub flags_placed: u32,
    pub start_time: Option<std::time::Instant>,
    pub elapsed_seconds: u32,
}

impl Minesweeper {
    pub fn new(difficulty: Difficulty) -> Self {
        let (rows, cols, mines) = difficulty.config();
        Self {
            rows,
            cols,
            mines,
            cells: vec![Cell::new(); (rows * cols) as usize],
            status: GameStatus::NotStarted,
            flags_placed: 0,
            start_time: None,
            elapsed_seconds: 0,
        }
    }

    pub fn reset(&mut self, difficulty: Difficulty) {
        *self = Self::new(difficulty);
    }

    pub fn index(&self, row: u32, col: u32) -> usize {
        (row * self.cols + col) as usize
    }

    // Made public for the view to use in rendering chording
    pub fn neighbors(&self, row: u32, col: u32) -> Vec<(u32, u32)> {
        let mut neighbors = Vec::with_capacity(8);
        for dr in -1..=1 {
            for dc in -1..=1 {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let nr = row as i32 + dr;
                let nc = col as i32 + dc;
                if nr >= 0 && nr < self.rows as i32 && nc >= 0 && nc < self.cols as i32 {
                    neighbors.push((nr as u32, nc as u32));
                }
            }
        }
        neighbors
    }

    fn place_mines(&mut self, safe_row: u32, safe_col: u32) {
        let total_cells = self.rows * self.cols;
        let safe_index = self.index(safe_row, safe_col);

        let mut indices: Vec<usize> = (0..total_cells as usize).collect();
        // Remove safe index from possible mine locations to ensure first click is safe
        if let Some(pos) = indices.iter().position(|&x| x == safe_index) {
            indices.swap_remove(pos);
        }

        let mut rng = rng();
        indices.shuffle(&mut rng);

        let mine_indices = &indices[0..self.mines as usize];
        for &idx in mine_indices {
            self.cells[idx].content = CellContent::Mine;
        }

        // Calculate numbers
        for r in 0..self.rows {
            for c in 0..self.cols {
                let idx = self.index(r, c);
                if self.cells[idx].content == CellContent::Mine {
                    continue;
                }
                let count = self
                    .neighbors(r, c)
                    .iter()
                    .filter(|&&(nr, nc)| {
                        self.cells[self.index(nr, nc)].content == CellContent::Mine
                    })
                    .count();
                if count > 0 {
                    self.cells[idx].content = CellContent::Number(count as u8);
                }
            }
        }
    }

    pub fn reveal(&mut self, row: u32, col: u32) {
        if self.status == GameStatus::Won || self.status == GameStatus::Lost {
            return;
        }

        if self.status == GameStatus::NotStarted {
            self.status = GameStatus::Playing;
            self.start_time = Some(std::time::Instant::now());
            self.place_mines(row, col);
        }

        let idx = self.index(row, col);
        let cell = &mut self.cells[idx];

        if cell.state == CellState::Flagged || cell.state == CellState::Revealed {
            return;
        }

        cell.state = CellState::Revealed;

        match cell.content {
            CellContent::Mine => {
                self.status = GameStatus::Lost;
                cell.exploded = true;
                self.reveal_all_mines();
            }
            CellContent::Empty => {
                // Flood fill
                let mut stack = vec![(row, col)];
                while let Some((r, c)) = stack.pop() {
                    for (nr, nc) in self.neighbors(r, c) {
                        let n_idx = self.index(nr, nc);
                        if self.cells[n_idx].state == CellState::Hidden {
                            self.cells[n_idx].state = CellState::Revealed;
                            if self.cells[n_idx].content == CellContent::Empty {
                                stack.push((nr, nc));
                            }
                        }
                    }
                }
                self.check_win();
            }
            CellContent::Number(_) => {
                self.check_win();
            }
        }
    }

    pub fn toggle_flag(&mut self, row: u32, col: u32) {
        if self.status != GameStatus::Playing && self.status != GameStatus::NotStarted {
            return;
        }
        let idx = self.index(row, col);
        let cell = &mut self.cells[idx];
        match cell.state {
            CellState::Hidden => {
                cell.state = CellState::Flagged;
                self.flags_placed += 1;
            }
            CellState::Flagged => {
                cell.state = CellState::QuestionMark;
                self.flags_placed -= 1;
            }
            CellState::QuestionMark => {
                cell.state = CellState::Hidden;
            }
            _ => {}
        }
    }

    pub fn chord(&mut self, row: u32, col: u32) -> bool {
        if self.status != GameStatus::Playing {
            return false;
        }
        let idx = self.index(row, col);
        if self.cells[idx].state != CellState::Revealed {
            return false;
        }

        if let CellContent::Number(n) = self.cells[idx].content {
            let neighbors = self.neighbors(row, col);
            let flag_count = neighbors
                .iter()
                .filter(|&&(nr, nc)| self.cells[self.index(nr, nc)].state == CellState::Flagged)
                .count();

            if flag_count == n as usize {
                for (nr, nc) in neighbors {
                    if self.cells[self.index(nr, nc)].state == CellState::Hidden
                        || self.cells[self.index(nr, nc)].state == CellState::QuestionMark
                    {
                        self.reveal(nr, nc);
                    }
                }
                return true;
            }
        }
        false
    }

    fn reveal_all_mines(&mut self) {
        for i in 0..self.cells.len() {
            if self.cells[i].content == CellContent::Mine
                && self.cells[i].state != CellState::Flagged
            {
                self.cells[i].state = CellState::Revealed;
            }
            // Check for wrong flags
            if self.cells[i].content != CellContent::Mine
                && self.cells[i].state == CellState::Flagged
            {
                self.cells[i].wrong_flag = true;
                self.cells[i].state = CellState::Revealed; // Show it was wrong
            }
        }
    }

    fn check_win(&mut self) {
        let total_cells = self.rows * self.cols;
        let revealed_count = self
            .cells
            .iter()
            .filter(|c| c.state == CellState::Revealed)
            .count();
        if revealed_count as u32 == total_cells - self.mines {
            self.status = GameStatus::Won;
            self.flag_all_mines();
        }
    }

    fn flag_all_mines(&mut self) {
        self.flags_placed = 0;
        for cell in &mut self.cells {
            if cell.content == CellContent::Mine {
                cell.state = CellState::Flagged;
                self.flags_placed += 1;
            }
        }
        // Actually, in WinXP, the flag count matches mines when won.
        self.flags_placed = self.mines;
    }
}
