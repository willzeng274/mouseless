use eframe::egui;

pub const MAIN_GRID_COLS: usize = 12;
pub const MAIN_GRID_ROWS: usize = 12;
pub const SUB_GRID_COLS: usize = 5;
pub const SUB_GRID_ROWS: usize = 5;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum DisplayMode {
    MainGrid,
    SubGrid,
}

pub fn generate_main_grid_layout(num_cols: usize, num_rows: usize, screen_rect: egui::Rect) -> (Vec<String>, Vec<egui::Rect>) {
    let mut labels = Vec::new();
    let first_chars = ['A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', 'Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P', 'Z', 'X', 'C', 'V', 'B', 'N', 'M'];
    let second_chars = ['H', 'J', 'K', 'L', 'Q', 'W', 'E', 'R', 'T', 'Y', 'A', 'S', 'D', 'F', 'G', 'U', 'I', 'O', 'P', 'Z', 'X', 'C', 'V', 'B', 'N', 'M'];
    for r in 0..num_rows {
        for c in 0..num_cols {
            let idx = r * num_cols + c;
            if idx < first_chars.len() && idx < second_chars.len() {
                let char1 = first_chars[r % first_chars.len()];
                let char2 = second_chars[c % second_chars.len()];
                if labels.len() < num_rows * num_cols {
                    labels.push(format!("{}{}", char1, char2));
                }
            } else {
                if labels.len() < num_rows * num_cols {
                     labels.push(format!("{}{}", (65 + (idx % 26)) as u8 as char, (65 + ((idx / 26) % 26)) as u8 as char));
                }
            }
        }
    }
    labels.truncate(num_rows * num_cols);
    let mut rects = Vec::with_capacity(num_rows * num_cols);
    if screen_rect.width() > 1.0 && screen_rect.height() > 1.0 {
        let cell_width = screen_rect.width() / num_cols as f32;
        let cell_height = screen_rect.height() / num_rows as f32;
        for i in 0..num_rows {
            for j in 0..num_cols {
                rects.push(egui::Rect::from_min_size(
                    screen_rect.min + egui::vec2(j as f32 * cell_width, i as f32 * cell_height),
                    egui::vec2(cell_width, cell_height)
                ));
            }
        }
    }
    (labels, rects)
}

pub fn generate_sub_grid_layout(main_cell_rect: egui::Rect, num_cols: usize, num_rows: usize) -> (Vec<String>, Vec<egui::Rect>) {
    let mut labels = Vec::new();
    let sub_grid_chars = [
        'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
        'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    ];
    let total_cells = num_cols * num_rows;
    for i in 0..total_cells {
        if i < sub_grid_chars.len() {
            labels.push(sub_grid_chars[i].to_string());
        }
    }
    labels.truncate(total_cells);
    let mut rects = Vec::with_capacity(total_cells);
    if main_cell_rect.width() > 1.0 && main_cell_rect.height() > 1.0 {
        let cell_width = main_cell_rect.width() / num_cols as f32;
        let cell_height = main_cell_rect.height() / num_rows as f32;
        for r in 0..num_rows {
            for c in 0..num_cols {
                rects.push(egui::Rect::from_min_size(
                    main_cell_rect.min + egui::vec2(c as f32 * cell_width, r as f32 * cell_height),
                    egui::vec2(cell_width, cell_height)
                ));
            }
        }
    }
    (labels, rects)
} 