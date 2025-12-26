#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui::*;
use std::time::Duration;

mod game;
use game::{Cell, CellContent, CellState, Difficulty, GameStatus, Minesweeper};

actions!(
    minesweeper,
    [NewGame, DiffBeginner, DiffIntermediate, DiffExpert, Exit]
);

struct MinesweeperView {
    game: Minesweeper,
    difficulty: Difficulty,
    timer_handle: Option<Task<()>>,
    chord_target: Option<(u32, u32)>, // Track which cell is being chorded (pressed)
    flashing_cells: Vec<(u32, u32)>,  // For visual feedback on failed chords
    left_mouse_down: bool,
    right_mouse_down: bool,
}

impl MinesweeperView {
    fn new(cx: &mut Context<Self>) -> Self {
        let difficulty = Difficulty::Beginner;
        let mut view = Self {
            game: Minesweeper::new(difficulty),
            difficulty,
            timer_handle: None,
            chord_target: None,
            flashing_cells: Vec::new(),
            left_mouse_down: false,
            right_mouse_down: false,
        };
        view.start_timer(cx);
        view
    }

    fn start_timer(&mut self, cx: &mut Context<Self>) {
        if self.timer_handle.is_some() {
            return;
        }
        self.timer_handle = Some(cx.spawn(
            |view: WeakEntity<MinesweeperView>, cx: &mut AsyncApp| {
                let mut cx_owned = cx.clone();
                async move {
                    loop {
                        cx_owned
                            .background_executor()
                            .timer(Duration::from_secs(1))
                            .await;
                        if view
                            .update(
                                &mut cx_owned,
                                |view: &mut MinesweeperView, cx: &mut Context<MinesweeperView>| {
                                    if view.game.status == GameStatus::Playing {
                                        view.game.elapsed_seconds += 1;
                                        if view.game.elapsed_seconds > 999 {
                                            view.game.elapsed_seconds = 999;
                                        }
                                        cx.notify();
                                    }
                                },
                            )
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            },
        ));
    }

    fn handle_click(&mut self, row: u32, col: u32, cx: &mut Context<Self>) {
        self.game.reveal(row, col);
        cx.notify();
    }

    fn handle_right_click(&mut self, row: u32, col: u32, cx: &mut Context<Self>) {
        self.game.toggle_flag(row, col);
        cx.notify();
    }

    fn handle_chord_start(&mut self, row: u32, col: u32, cx: &mut Context<Self>) {
        self.chord_target = Some((row, col));
        cx.notify();
    }

    fn handle_chord_end(&mut self, row: u32, col: u32, cx: &mut Context<Self>) {
        if self.chord_target == Some((row, col)) {
            let success = self.game.chord(row, col);
            self.chord_target = None;

            if !success {
                // Flash neighbors
                let neighbors = self.game.neighbors(row, col);
                self.flashing_cells = neighbors
                    .into_iter()
                    .filter(|&(nr, nc)| {
                        let state = self.game.cells[self.game.index(nr, nc)].state;
                        state == CellState::Hidden || state == CellState::QuestionMark
                    })
                    .collect();

                cx.spawn(|view: WeakEntity<MinesweeperView>, cx: &mut AsyncApp| {
                    let mut cx = cx.clone();
                    async move {
                        cx.background_executor()
                            .timer(Duration::from_millis(150))
                            .await;
                        view.update(
                            &mut cx,
                            |view: &mut MinesweeperView, cx: &mut Context<MinesweeperView>| {
                                view.flashing_cells.clear();
                                cx.notify();
                            },
                        )
                        .ok();
                    }
                })
                .detach();
            }

            cx.notify();
        }
    }

    fn handle_chord_cancel(&mut self, cx: &mut Context<Self>) {
        if self.chord_target.is_some() {
            self.chord_target = None;
            cx.notify();
        }
    }

    fn reset(&mut self, difficulty: Difficulty, cx: &mut Context<Self>) {
        self.difficulty = difficulty;
        self.game.reset(difficulty);
        cx.notify();

        // Resize window based on difficulty
        // let (rows, cols, _) = difficulty.config();
        // let width = cols as f32 * 24.0 + 40.0; // Approximate
        // let height = rows as f32 * 24.0 + 100.0; // Approximate

        // TODO: Resize window not supported in current gpui version or requires different API
        /*
        cx.resize_window(WindowSize {
            width: px(width),
            height: px(height),
        });
        */
    }
}

// Colors
fn color_gray() -> Rgba {
    rgba(0xC0C0C0FF)
} // #C0C0C0
fn color_white() -> Rgba {
    rgba(0xFFFFFFFF)
}
fn color_dark_gray() -> Rgba {
    rgba(0x808080FF)
} // #808080
fn color_black() -> Rgba {
    rgba(0x000000FF)
}
fn color_red() -> Rgba {
    rgba(0xFF0000FF)
}

// Helper for bevels
fn bevel_raised(content: Div) -> Div {
    // Simulate raised bevel: Light Top/Left, Dark Bottom/Right (3px for window/panels)
    div().bg(color_dark_gray()).pb(px(3.0)).pr(px(3.0)).child(
        div()
            .bg(color_white())
            .pt(px(3.0))
            .pl(px(3.0))
            .child(content.bg(color_gray())),
    )
}

fn bevel_sunken(content: Div) -> Div {
    // Simulate sunken bevel: Dark Top/Left, White Bottom/Right (3px)
    div().bg(color_white()).pb(px(3.0)).pr(px(3.0)).child(
        div()
            .bg(color_dark_gray())
            .pt(px(3.0))
            .pl(px(3.0))
            .child(content.bg(color_gray())),
    )
}

fn bevel_sunken_thin(content: Div) -> Div {
    // Thinner sunken bevel for counters (1px or 2px)
    // The background inside this bevel should be BLACK.
    div().bg(color_white()).pb(px(1.0)).pr(px(1.0)).child(
        div()
            .bg(color_dark_gray())
            .pt(px(1.0))
            .pl(px(1.0))
            .child(content.bg(color_black())), // Ensure content bg is black
    )
}

impl Render for MinesweeperView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (rows, cols) = (self.game.rows, self.game.cols);
        let status = self.game.status;

        // Collect grid children using for loops to avoid closure capturing issues
        let mut grid = Vec::with_capacity(rows as usize);
        for r in 0..rows {
            let mut row_children = Vec::with_capacity(cols as usize);
            for c in 0..cols {
                let idx = (r * cols + c) as usize;
                let cell = &self.game.cells[idx];
                row_children.push(self.render_cell(r, c, cell, cx));
            }
            grid.push(div().flex().flex_row().children(row_children));
        }

        let mines_left = self.game.mines as i32 - self.game.flags_placed as i32;
        let mines_display = format!("{:03}", mines_left.max(-99).min(999));
        let time_display = format!("{:03}", self.game.elapsed_seconds);

        div()
            .key_context("Minesweeper")
            .on_action(
                cx.listener(|view, _: &NewGame, _window, cx| view.reset(view.difficulty, cx)),
            )
            .on_action(cx.listener(|view, _: &DiffBeginner, _window, cx| {
                view.reset(Difficulty::Beginner, cx)
            }))
            .on_action(cx.listener(|view, _: &DiffIntermediate, _window, cx| {
                view.reset(Difficulty::Intermediate, cx)
            }))
            .on_action(
                cx.listener(|view, _: &DiffExpert, _window, cx| view.reset(Difficulty::Expert, cx)),
            )
            .on_action(cx.listener(|_, _: &Exit, _window, cx| cx.quit()))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, _window, cx| view.handle_chord_cancel(cx)),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|view, _, _window, cx| view.handle_chord_cancel(cx)),
            )
            .flex()
            .flex_col()
            .bg(color_gray())
            .w_full() // Ensure it fills the width
            .h_full() // Ensure it fills the height
            .p(px(6.0))
            .gap(px(6.0))
            .child(bevel_raised(
                div()
                    .flex()
                    .flex_col()
                    .p(px(6.0))
                    .gap(px(6.0))
                    .child(
                        // Header
                        bevel_sunken(
                            // Use thick bevel for the header container? Actually usually header and board are separate sunken areas.
                            // In Win2000, there's just a sunken border around the board, and the counters are sunken.
                            // The container holding counters is FLUSH with the gray background.
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .p(px(4.0))
                                .child(
                                    // Mine Counter
                                    bevel_sunken_thin(
                                        div()
                                            .text_color(color_red())
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(24.0))
                                            .font_family("Courier New") // Monospace
                                            .child(mines_display),
                                    ),
                                )
                                .child(
                                    // Smiley Face Button
                                    div().w(px(26.0)).h(px(26.0)).child(
                                        // Make the button itself a bevel (raised)
                                        // Button usually has 2px bevel
                                        div().bg(color_dark_gray()).pb(px(2.0)).pr(px(2.0)).child(
                                            div().bg(color_white()).pt(px(2.0)).pl(px(2.0)).child(
                                                div()
                                                    .w(px(22.0))
                                                    .h(px(22.0))
                                                    .bg(color_gray())
                                                    .flex()
                                                    .justify_center()
                                                    .items_center()
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(|view, _, _window, cx| {
                                                            let d = view.difficulty;
                                                            view.reset(d, cx);
                                                        }),
                                                    )
                                                    .child(match status {
                                                        GameStatus::Won => "😎",
                                                        GameStatus::Lost => "😵",
                                                        _ => "🙂",
                                                    }),
                                            ),
                                        ),
                                    ),
                                )
                                .child(
                                    // Timer
                                    bevel_sunken_thin(
                                        div()
                                            .text_color(color_red())
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(24.0))
                                            .font_family("Courier New")
                                            .child(time_display),
                                    ),
                                ),
                        ),
                    )
                    .child(
                        // Board
                        bevel_sunken(div().flex().flex_col().children(grid)),
                    ),
            ))
    }
}

impl MinesweeperView {
    fn render_cell(&self, row: u32, col: u32, cell: &Cell, cx: &Context<Self>) -> Div {
        let cell_size = px(16.0);

        let mut cell_div = div()
            .w(cell_size)
            .h(cell_size)
            .flex()
            .justify_center()
            .items_center()
            .text_size(px(14.0)) // Slightly smaller text for 16px cells
            .font_weight(FontWeight::BOLD);

        if let CellState::Revealed = cell.state {
            if let CellContent::Number(_) = cell.content {
                cell_div = cell_div.font_family("Times New Roman"); // Serif for numbers
            }
        }

        // Determine if this cell should be visually pressed (revealed style but empty)
        // This happens if it is targeted by a chord action or is a neighbor of a targeted chord action
        let mut visually_pressed = false;
        if let Some((t_row, t_col)) = self.chord_target {
            // Check if this cell is a neighbor of the target
            // We need to calculate neighbors here or assume the view knows.
            // Since we can't easily call self.game.neighbors() inside render loop efficiently without refactoring,
            // we will do a quick check.
            let is_neighbor =
                (row as i32 - t_row as i32).abs() <= 1 && (col as i32 - t_col as i32).abs() <= 1;

            if is_neighbor
                && (cell.state == CellState::Hidden || cell.state == CellState::QuestionMark)
            {
                visually_pressed = true;
            }
        }

        if self.flashing_cells.contains(&(row, col)) {
            visually_pressed = true;
        }

        if visually_pressed {
            // Render as pressed (Revealed style but empty content for now)
            cell_div = cell_div
                .bg(color_gray())
                .border(px(1.0))
                .border_color(color_dark_gray());
            // No content for pressed state unless we want to show something?
            // In Win2000, it just looks like an empty revealed cell.
            return cell_div;
        }

        match cell.state {
            CellState::Hidden | CellState::Flagged | CellState::QuestionMark => {
                // Manual bevel for cell to keep it efficient and tight
                cell_div = cell_div
                    .bg(color_dark_gray()) // Shadow Bottom/Right
                    .pb(px(2.0))
                    .pr(px(2.0))
                    .child(
                        div()
                            .w_full()
                            .h_full()
                            .bg(color_white()) // Highlight Top/Left
                            .pt(px(2.0))
                            .pl(px(2.0))
                            .child(
                                div()
                                    .w_full()
                                    .h_full()
                                    .bg(color_gray())
                                    .flex()
                                    .text_size(px(12.0))
                                    .justify_center()
                                    .items_center()
                                    .child(match cell.state {
                                        CellState::Flagged => "⛳", // Triangular flag is closer to Windows style
                                        // A unicode flag is the best we can do without custom assets.
                                        // The Win2000 flag is red triangle on black pole.
                                        // "⛳" (Triangular Flag on Post) is usually red.
                                        // Let's try to just change color if needed, but unicode color is fixed.
                                        // We could draw it with divs but that's complex.
                                        // Let's stick with the unicode but maybe make it smaller or different if possible.
                                        // Actually, let's just keep the unicode for now as drawing a flag with divs is overkill.
                                        CellState::QuestionMark => "?",
                                        _ => "",
                                    }),
                            ),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _window, cx| {
                            view.handle_click(row, col, cx);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |view, _, _window, cx| {
                            view.handle_right_click(row, col, cx);
                        }),
                    );
            }
            CellState::Revealed => {
                cell_div = cell_div
                    .bg(color_gray())
                    .border(px(1.0)) // Add faint border to simulate grid lines
                    .border_color(color_dark_gray());

                if let CellContent::Number(_) = cell.content {
                    cell_div = cell_div
                        .on_mouse_down(
                            MouseButton::Middle,
                            cx.listener(move |view, _, _window, cx| {
                                view.handle_chord_start(row, col, cx);
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Middle,
                            cx.listener(move |view, _, _window, cx| {
                                view.handle_chord_end(row, col, cx);
                            }),
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, event: &MouseDownEvent, _window, cx| {
                                view.left_mouse_down = true;
                                if event.click_count == 2 {
                                    view.handle_chord_start(row, col, cx);
                                } else if view.right_mouse_down {
                                    view.handle_chord_start(row, col, cx);
                                }
                            }),
                        )
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |view, _, _window, cx| {
                                view.right_mouse_down = true;
                                if view.left_mouse_down {
                                    view.handle_chord_start(row, col, cx);
                                }
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _, _window, cx| {
                                view.left_mouse_down = false;
                                // If we were chording, finish it.
                                view.handle_chord_end(row, col, cx);
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Right,
                            cx.listener(move |view, _, _window, cx| {
                                view.right_mouse_down = false;
                                // If we were chording, finish it.
                                view.handle_chord_end(row, col, cx);
                            }),
                        );
                }

                let content = match cell.content {
                    CellContent::Empty => "",
                    CellContent::Mine => {
                        if cell.exploded {
                            "💥"
                        } else {
                            "💣"
                        }
                    }
                    CellContent::Number(n) => match n {
                        1 => "1",
                        2 => "2",
                        3 => "3",
                        4 => "4",
                        5 => "5",
                        6 => "6",
                        7 => "7",
                        8 => "8",
                        _ => "",
                    },
                };

                let color = match cell.content {
                    CellContent::Number(n) => match n {
                        1 => rgba(0x0000FFFF), // Blue
                        2 => rgba(0x008000FF), // Green
                        3 => rgba(0xFF0000FF), // Red
                        4 => rgba(0x000080FF), // Dark Blue
                        5 => rgba(0x800000FF), // Maroon
                        6 => rgba(0x008080FF), // Teal
                        7 => rgba(0x000000FF), // Black
                        8 => rgba(0x808080FF), // Gray
                        _ => color_black(),
                    },
                    _ => color_black(),
                };

                let mut inner = cell_div.text_color(color);
                if cell.content == CellContent::Mine && cell.exploded {
                    inner = inner.bg(color_red());
                }
                cell_div = inner.child(content);
            }
        }

        cell_div
    }
}

fn main() {
    Application::new().run(|cx| {
        cx.set_menus(vec![Menu {
            name: "Game".into(),
            items: vec![
                MenuItem::action("New", NewGame),
                MenuItem::separator(),
                MenuItem::action("Beginner", DiffBeginner),
                MenuItem::action("Intermediate", DiffIntermediate),
                MenuItem::action("Expert", DiffExpert),
                MenuItem::separator(),
                MenuItem::action("Exit", Exit),
            ],
        }]);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(180.0), px(240.0)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Minesweeper".into()),
                appears_transparent: false,
                traffic_light_position: Some(point(px(8.0), px(8.0))),
            }),
            ..Default::default()
        };
        let _ = cx.open_window(options, |_, cx| {
            // Quit the app when a window is closed
            cx.on_window_closed(|cx| {
                cx.quit();
            })
            .detach();
            cx.new(|cx| MinesweeperView::new(cx))
        });

        cx.activate(true); // Bring to front
    });
}
