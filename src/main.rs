#![allow(unexpected_cfgs)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::{Instant, Duration};
use std::ptr;
use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::cell::Cell;

use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoopSource, CFRunLoop};
use core_foundation::mach_port::CFMachPortCreateRunLoopSource;
use core_foundation::base::TCFType;
use core_graphics::event::{
    CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy, CGEventType,
    CGEventFlags, CGEvent, EventField, CGEventTap, CGMouseButton
};
use core_graphics::geometry::CGPoint;
use core_graphics::event_source::CGEventSourceStateID;
use eframe::{egui, NativeOptions};
use objc::{msg_send, sel, sel_impl, class};
use objc::runtime::Object;
use mouse_rs::{Mouse};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(target_os = "macos")]
use cocoa::appkit::{NSWindowCollectionBehavior, NSWindowStyleMask};
#[cfg(target_os = "macos")]
const NSNONACTIVATING_PANEL_MASK: u64 = 1 << 7;

const RCMD_TAP_DURATION_MS: u128 = 200;
const RIGHT_COMMAND_KEY_CODE: i64 = 54;
const LEFT_SHIFT_KEY_CODE: i64 = 56;
const ESCAPE_KEY_CODE: i64 = 53;
const MAIN_GRID_COLS: usize = 12;
const MAIN_GRID_ROWS: usize = 12;
const SUB_GRID_COLS: usize = 5;
const SUB_GRID_ROWS: usize = 5;

#[derive(Debug)]
enum GlobalEvent {
    ShowGrid(Option<egui::Pos2>),
}

struct EventTapSharedState {
    event_tx: Sender<GlobalEvent>,
    app_is_visible: Arc<AtomicBool>,
    eframe_hide_requested_by_listener: Arc<AtomicBool>,
    lshift_key_is_pressed: Arc<AtomicBool>,
}

#[derive(Clone)]
struct EframeControl {
    hide_requested: Arc<AtomicBool>,
    is_visible: Arc<AtomicBool>,
}

impl Default for EframeControl {
    fn default() -> Self {
        Self {
            hide_requested: Arc::new(AtomicBool::new(false)),
            is_visible: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum DisplayMode {
    MainGrid,
    SubGrid,
}

fn key_to_char(key: egui::Key, _modifiers: egui::Modifiers) -> Option<char> {
    match key {
        egui::Key::A => Some('A'), egui::Key::B => Some('B'), egui::Key::C => Some('C'),
        egui::Key::D => Some('D'), egui::Key::E => Some('E'), egui::Key::F => Some('F'),
        egui::Key::G => Some('G'), egui::Key::H => Some('H'), egui::Key::I => Some('I'),
        egui::Key::J => Some('J'), egui::Key::K => Some('K'), egui::Key::L => Some('L'),
        egui::Key::M => Some('M'), egui::Key::N => Some('N'), egui::Key::O => Some('O'),
        egui::Key::P => Some('P'), egui::Key::Q => Some('Q'), egui::Key::R => Some('R'),
        egui::Key::S => Some('S'), egui::Key::T => Some('T'), egui::Key::U => Some('U'),
        egui::Key::V => Some('V'), egui::Key::W => Some('W'), egui::Key::X => Some('X'),
        egui::Key::Y => Some('Y'), egui::Key::Z => Some('Z'),
        _ => None,
    }
}

struct MouselessApp {
    display_mode: DisplayMode,
    key_input_buffer: String,
    selected_main_cell_index: Option<usize>,
    main_grid_labels: Vec<String>,
    main_grid_rects: Vec<egui::Rect>,
    sub_grid_labels: Vec<String>,
    sub_grid_rects: Vec<egui::Rect>,
    last_layout_screen_rect: egui::Rect,
    mouse_handler: Mouse,
    eframe_control: EframeControl,
    _initial_target_rect: egui::Rect,
    initial_focus_requested: bool,
    #[cfg(target_os = "macos")]
    macos_panel_properties_set: bool,
    event_rx: Receiver<GlobalEvent>,
    lshift_key_is_pressed: Arc<AtomicBool>,
    is_hiding_to_perform_click: bool,
    hide_initiated_at: Option<Instant>,
    pending_click_pos_after_hide: Option<egui::Pos2>,
}

impl MouselessApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        eframe_control: EframeControl,
        initial_target_rect: egui::Rect,
        event_rx: Receiver<GlobalEvent>,
        lshift_key_is_pressed: Arc<AtomicBool>,
    ) -> Self {
        let (labels, _) = Self::generate_main_grid_layout(
            MAIN_GRID_COLS,
            MAIN_GRID_ROWS,
            egui::Rect::from_min_size(egui::Pos2::ZERO, initial_target_rect.size()),
        );
        
        let s = Self {
            display_mode: DisplayMode::MainGrid,
            key_input_buffer: String::new(),
            selected_main_cell_index: None,
            main_grid_labels: labels,
            main_grid_rects: Vec::new(),
            sub_grid_labels: Vec::new(),
            sub_grid_rects: Vec::new(),
            last_layout_screen_rect: egui::Rect::NOTHING,
            mouse_handler: Mouse::new(),
            eframe_control,
            _initial_target_rect: initial_target_rect,
            initial_focus_requested: false,
            #[cfg(target_os = "macos")]
            macos_panel_properties_set: false,
            event_rx,
            lshift_key_is_pressed,
            is_hiding_to_perform_click: false,
            hide_initiated_at: None,
            pending_click_pos_after_hide: None,
        };

        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals.window_fill = egui::Color32::TRANSPARENT;
        style.visuals.panel_fill = egui::Color32::TRANSPARENT;
        cc.egui_ctx.set_style(style);
        s
    }

    fn generate_main_grid_layout(num_cols: usize, num_rows: usize, screen_rect: egui::Rect) -> (Vec<String>, Vec<egui::Rect>) {
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

    fn generate_sub_grid_layout(main_cell_rect: egui::Rect, num_cols: usize, num_rows: usize) -> (Vec<String>, Vec<egui::Rect>) {
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
    
    fn perform_mouse_click(&mut self, _ctx: &egui::Context, window_relative_point: egui::Pos2) {
        let current_viewport_outer_rect = _ctx.input(|i| i.viewport().outer_rect);
        if let Some(window_outer_rect) = current_viewport_outer_rect {
            let window_origin_global = window_outer_rect.min;
            let global_click_point = window_origin_global + window_relative_point.to_vec2();

            println!("Preparing click at {:?}", global_click_point);

            if let Err(e) = self.mouse_handler.move_to(global_click_point.x as i32, global_click_point.y as i32) {
                eprintln!("Failed to move mouse: {:?}", e);
                self.eframe_control.hide_requested.store(true, AtomicOrdering::SeqCst);
                self.pending_click_pos_after_hide = None;
                return;
            } else {
                println!("Mouse moved to ({}, {})", global_click_point.x as i32, global_click_point.y as i32);
            }
            
            self.pending_click_pos_after_hide = Some(global_click_point);
            println!("Click queued, hiding app");

        } else {
            eprintln!("Failed to get window rect for click at {:?}", window_relative_point);
            self.pending_click_pos_after_hide = None;
        }
        self.eframe_control.hide_requested.store(true, AtomicOrdering::SeqCst);
    }
}

impl eframe::App for MouselessApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) { 
        if let Ok(event) = self.event_rx.try_recv() {
            match event {
                GlobalEvent::ShowGrid(_cursor_pos_opt) => {
                    if !self.eframe_control.is_visible.load(AtomicOrdering::SeqCst) {
                        println!("Showing grid");
                        self.eframe_control.is_visible.store(true, AtomicOrdering::SeqCst);
                        self.eframe_control.hide_requested.store(false, AtomicOrdering::SeqCst);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus); 
                        self.initial_focus_requested = true;
                        self.key_input_buffer.clear();
                        self.selected_main_cell_index = None;
                        self.display_mode = DisplayMode::MainGrid;
                        self.main_grid_rects.clear();
                    }
                }
            }
        }

        let hide_req = self.eframe_control.hide_requested.load(AtomicOrdering::SeqCst);
        if hide_req {
            if self.eframe_control.is_visible.load(AtomicOrdering::SeqCst) {
                println!("Hiding window");
                self.eframe_control.is_visible.store(false, AtomicOrdering::SeqCst);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.eframe_control.hide_requested.store(false, AtomicOrdering::SeqCst);
                self.key_input_buffer.clear();
                self.selected_main_cell_index = None;
                self.display_mode = DisplayMode::MainGrid;
                println!("Hide initiated");
                self.is_hiding_to_perform_click = self.pending_click_pos_after_hide.is_some();
                if self.is_hiding_to_perform_click {
                    self.hide_initiated_at = Some(Instant::now());
                }
                return;
            }
            else if hide_req { 
                 self.eframe_control.hide_requested.store(false, AtomicOrdering::SeqCst);
                 if self.pending_click_pos_after_hide.is_some() {
                    println!("Clearing pending click");
                    self.pending_click_pos_after_hide = None;
                 }
                 self.is_hiding_to_perform_click = false;
                 self.hide_initiated_at = None;
            }
        }

        if self.is_hiding_to_perform_click {
            if let Some(initiated_at) = self.hide_initiated_at {
                if initiated_at.elapsed() >= Duration::from_millis(150) {
                    if let Some(pos_to_click) = self.pending_click_pos_after_hide.take() {
                        println!("Performing click at {:?}", pos_to_click);
                        
                        #[cfg(target_os = "macos")]
                        let mut ns_window_ptr_for_mouse_ignore: *mut Object = std::ptr::null_mut();
                        
                        #[cfg(target_os = "macos")]
                        match frame.window_handle() { 
                            Ok(handle) => match handle.as_raw() {
                                RawWindowHandle::AppKit(app_kit_handle) => {
                                    let view_ptr = app_kit_handle.ns_view.as_ptr() as *mut Object;
                                    unsafe {
                                        let window_ptr: *mut Object = msg_send![view_ptr, window];
                                        if !window_ptr.is_null() {
                                            ns_window_ptr_for_mouse_ignore = window_ptr;
                                            let _:() = msg_send![ns_window_ptr_for_mouse_ignore, setIgnoresMouseEvents:true];
                                            println!("Window set to ignore mouse events");
                                        }
                                    }
                                }
                                _ => {}
                            }
                            Err(_) => {}
                        }

                        let click_point_cg = CGPoint::new(pos_to_click.x as f64, pos_to_click.y as f64);
                        let (mouse_down_event_type, mouse_up_event_type, button_for_log) = 
                            if self.lshift_key_is_pressed.load(AtomicOrdering::SeqCst) {
                                println!("Using right click (shift held)");
                                (CGEventType::RightMouseDown, CGEventType::RightMouseUp, "Right")
                            } else {
                                println!("Using left click");
                                (CGEventType::LeftMouseDown, CGEventType::LeftMouseUp, "Left")
                            };
                        let mouse_button_to_use = if button_for_log == "Right" { CGMouseButton::Right } else { CGMouseButton::Left };

                        match core_graphics::event_source::CGEventSource::new(CGEventSourceStateID::Private) {
                            Ok(event_source) => {
                                let mouse_down = CGEvent::new_mouse_event(event_source.clone(), mouse_down_event_type, click_point_cg, mouse_button_to_use);
                                let mouse_up = CGEvent::new_mouse_event(event_source, mouse_up_event_type, click_point_cg, mouse_button_to_use);

                                if let Ok(down_event) = mouse_down {
                                    down_event.post(CGEventTapLocation::HID);
                                    println!("Posted {} click down", button_for_log.to_lowercase());
                                } else { eprintln!("Failed to create {} click down event", button_for_log.to_lowercase()); }

                                if let Ok(up_event) = mouse_up {
                                    up_event.post(CGEventTapLocation::HID);
                                    println!("Posted {} click up", button_for_log.to_lowercase());
                                } else { eprintln!("Failed to create {} click up event", button_for_log.to_lowercase()); }
                            }
                            Err(e) => { eprintln!("Failed to create event source: {:?}", e); }
                        }
                        
                        #[cfg(target_os = "macos")]
                        if !ns_window_ptr_for_mouse_ignore.is_null() {
                            unsafe {
                                let _:() = msg_send![ns_window_ptr_for_mouse_ignore, setIgnoresMouseEvents:false];
                                println!("Window restored to normal mouse handling");
                            }
                        }
                    }
                    self.is_hiding_to_perform_click = false;
                    self.hide_initiated_at = None;
                    self.pending_click_pos_after_hide = None;
                    self.key_input_buffer.clear();
                    self.selected_main_cell_index = None;
                    self.display_mode = DisplayMode::MainGrid;
                    self.eframe_control.hide_requested.store(false, AtomicOrdering::SeqCst);
                    println!("Click sequence complete");
                } else {
                    ctx.request_repaint_after(Duration::from_millis(20)); 
                }
            } else { 
                self.is_hiding_to_perform_click = false;
                self.pending_click_pos_after_hide = None;
                self.eframe_control.hide_requested.store(false, AtomicOrdering::SeqCst);
            }
        }

        if !self.eframe_control.is_visible.load(AtomicOrdering::SeqCst) && !self.is_hiding_to_perform_click {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }

        #[cfg(target_os = "macos")]
        if !self.macos_panel_properties_set {
            match frame.window_handle() {
                Ok(handle) => match handle.as_raw() {
                    RawWindowHandle::AppKit(app_kit_handle) => {
                        let view_ptr = app_kit_handle.ns_view.as_ptr() as *mut Object;
                        unsafe {
                            let window_ptr: *mut Object = msg_send![view_ptr, window];
                            if !window_ptr.is_null() {
                                let collection_behavior = 
                                    NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces |
                                    NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary |
                                    NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary;
                                let _: () = msg_send![window_ptr, setCollectionBehavior: collection_behavior];
                                let current_style_mask: NSWindowStyleMask = msg_send![window_ptr, styleMask];
                                let new_style_mask = current_style_mask.bits() | NSNONACTIVATING_PANEL_MASK;
                                let _: () = msg_send![window_ptr, setStyleMask: NSWindowStyleMask::from_bits_truncate(new_style_mask)];
                                println!("Configured window as non-activating panel");
                                self.macos_panel_properties_set = true;
                            } else {
                            }
                        }
                    }
                    _ => { 
                    }
                }
                Err(_) => {
                }
            }
        }

        if !self.initial_focus_requested {
        }

        let current_content_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, ctx.screen_rect().size());
        if self.main_grid_rects.is_empty() || self.last_layout_screen_rect != current_content_rect {
            println!("Recalculating layout");
            let (labels, rects) = Self::generate_main_grid_layout(MAIN_GRID_COLS, MAIN_GRID_ROWS, current_content_rect);
            self.main_grid_labels = labels;
            self.main_grid_rects = rects;
            self.last_layout_screen_rect = current_content_rect;

            if self.display_mode == DisplayMode::SubGrid {
                 if let Some(main_idx) = self.selected_main_cell_index {
                    if main_idx < self.main_grid_rects.len() {
                        let selected_main_rect = self.main_grid_rects[main_idx];
                        let (sg_labels, sg_rects) = Self::generate_sub_grid_layout(selected_main_rect, SUB_GRID_COLS, SUB_GRID_ROWS);
                        self.sub_grid_labels = sg_labels;
                        self.sub_grid_rects = sg_rects;
                    } else { self.display_mode = DisplayMode::MainGrid; } 
                 } else { self.display_mode = DisplayMode::MainGrid; } 
            }
        }
        
        if self.display_mode == DisplayMode::MainGrid {
            let events = ctx.input(|i| i.events.clone());
            for event in events {
                if let egui::Event::Key { key, pressed: true, .. } = event {
                    if let Some(char_code) = key_to_char(key, Default::default()) {
                        self.key_input_buffer.push(char_code);
                        if self.key_input_buffer.len() == 2 {
                            if let Some(index) = self.main_grid_labels.iter().position(|label| *label == self.key_input_buffer) {
                                self.selected_main_cell_index = Some(index);
                                self.display_mode = DisplayMode::SubGrid;
                                self.key_input_buffer.clear();
                                 if let Some(main_idx) = self.selected_main_cell_index { 
                                    if main_idx < self.main_grid_rects.len() {
                                        let selected_main_rect = self.main_grid_rects[main_idx];
                                        let (sg_labels, sg_rects) = Self::generate_sub_grid_layout(selected_main_rect, SUB_GRID_COLS, SUB_GRID_ROWS);
                                        self.sub_grid_labels = sg_labels;
                                        self.sub_grid_rects = sg_rects;
                                    } else { self.display_mode = DisplayMode::MainGrid;}
                                 } else { self.display_mode = DisplayMode::MainGrid;}
                            } else { self.key_input_buffer.clear(); }
                        }
                    }
                }
            }
        } else if self.display_mode == DisplayMode::SubGrid {
            let events = ctx.input(|i| i.events.clone());
            for event in events {
                if let egui::Event::Key { key, pressed: true, .. } = event {
                    if key == egui::Key::Space { 
                        if let Some(main_idx) = self.selected_main_cell_index {
                            if main_idx < self.main_grid_rects.len() {
                                self.perform_mouse_click(ctx, self.main_grid_rects[main_idx].center());
                                break;
                            }
                        }
                    }
                    if let Some(char_code) = key_to_char(key, Default::default()) {
                        if let Some(sub_idx) = self.sub_grid_labels.iter().position(|label| *label == char_code.to_string()) {
                            if sub_idx < self.sub_grid_rects.len() {
                                self.perform_mouse_click(ctx, self.sub_grid_rects[sub_idx].center());
                                break;
                            }
                        }
                    }
                }
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let painter = ui.painter();
                let main_cell_bg_color = egui::Color32::from_rgba_unmultiplied(50, 50, 50, 180);
                let line_stroke = egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(220, 220, 220, 150));
                let text_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220);

                if !self.main_grid_rects.is_empty() {
                    for (index, rect) in self.main_grid_rects.iter().enumerate() {
                        let bg_color = if self.display_mode == DisplayMode::SubGrid && Some(index) != self.selected_main_cell_index {
                            egui::Color32::from_rgba_unmultiplied(30, 30, 30, 100)
                        } else { main_cell_bg_color };
                        painter.rect_filled(*rect, 0.0, bg_color);
                        painter.rect_stroke(*rect, 0.0, line_stroke);
                        if index < self.main_grid_labels.len() {
                            let cell_center = rect.center();
                            let font_size = rect.height().min(rect.width()) * 0.4;
                            painter.text(cell_center, egui::Align2::CENTER_CENTER, &self.main_grid_labels[index], egui::FontId::proportional(font_size), text_color);
                        }
                    }
                } else if self.display_mode == DisplayMode::MainGrid {
                     painter.text(ctx.screen_rect().center(), egui::Align2::CENTER_CENTER, "Waiting for layout...", egui::FontId::default(), text_color);
                }

                if self.display_mode == DisplayMode::SubGrid {
                    if self.sub_grid_rects.is_empty() && self.selected_main_cell_index.is_some() {
                         if let Some(idx) = self.selected_main_cell_index {
                            if idx < self.main_grid_rects.len() {
                                 let selected_rect = self.main_grid_rects[idx];
                                 painter.text(selected_rect.center(), egui::Align2::CENTER_CENTER, "Waiting for sub-layout...", egui::FontId::proportional(selected_rect.height() * 0.15), egui::Color32::YELLOW);
                            }
                        }
                    } else {
                        let sub_cell_bg_color = egui::Color32::from_rgba_unmultiplied(70, 70, 20, 220);
                        let sub_text_color = egui::Color32::WHITE;
                        for (index, rect) in self.sub_grid_rects.iter().enumerate() {
                            painter.rect_filled(*rect, 0.0, sub_cell_bg_color);
                            painter.rect_stroke(*rect, 0.0, line_stroke);
                            if index < self.sub_grid_labels.len() {
                                let cell_center = rect.center();
                                let font_size = rect.height().min(rect.width()) * 0.5;
                                painter.text(cell_center, egui::Align2::CENTER_CENTER, &self.sub_grid_labels[index], egui::FontId::proportional(font_size), sub_text_color);
                            }
                        }
                    }
                }
            });
        ctx.request_repaint();
    }
    
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}

fn global_event_listener_thread(shared_state: EventTapSharedState) {
    println!("Global event listener started");
    let rcmd_press_time: Cell<Option<Instant>> = Cell::new(None);
    let current_run_loop = CFRunLoop::get_current();

    let callback_closure = move |_proxy: CGEventTapProxy, event_type: CGEventType, event: &CGEvent| -> Option<CGEvent> {
        if shared_state.app_is_visible.load(AtomicOrdering::SeqCst) {
            match event_type {
                CGEventType::KeyDown => {
                    let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                    if key_code == ESCAPE_KEY_CODE {
                        println!("Escape pressed, hiding app");
                        shared_state.eframe_hide_requested_by_listener.store(true, AtomicOrdering::SeqCst);
                        return None;
                    }
                }
                _ => {}
            }
        }

        match event_type {
            CGEventType::FlagsChanged => {
                let flags = event.get_flags();
                let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);

                if key_code == RIGHT_COMMAND_KEY_CODE {
                    if flags.contains(CGEventFlags::CGEventFlagCommand) {
                        if rcmd_press_time.get().is_none() {
                            rcmd_press_time.set(Some(Instant::now()));
                        }
                    } else {
                        if let Some(press_time_instant) = rcmd_press_time.take() {
                            if press_time_instant.elapsed() < Duration::from_millis(RCMD_TAP_DURATION_MS as u64) {
                                if !shared_state.app_is_visible.load(AtomicOrdering::SeqCst) {
                                    println!("Right cmd tap detected, showing grid");
                                    let mouse = Mouse::new();
                                    let cursor_pos = match mouse.get_position() {
                                        Ok(point) => Some(egui::pos2(point.x as f32, point.y as f32)),
                                        Err(_) => None,
                                    };
                                    let _ = shared_state.event_tx.send(GlobalEvent::ShowGrid(cursor_pos));
                                } else {
                                     println!("Right cmd tap ignored, app already visible");
                                }
                            }
                        }
                    }
                } else if key_code == LEFT_SHIFT_KEY_CODE {
                    if flags.contains(CGEventFlags::CGEventFlagShift) {
                        if !shared_state.lshift_key_is_pressed.load(AtomicOrdering::SeqCst) {
                             shared_state.lshift_key_is_pressed.store(true, AtomicOrdering::SeqCst);
                             println!("Left shift pressed");
                        }
                    } else {
                        if shared_state.lshift_key_is_pressed.load(AtomicOrdering::SeqCst) {
                            shared_state.lshift_key_is_pressed.store(false, AtomicOrdering::SeqCst);
                            println!("Left shift released");
                        }
                    }
                } else {
                     if rcmd_press_time.get().is_some() { 
                        rcmd_press_time.set(None);
                     }
                }
            }
            CGEventType::KeyDown => {
                let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                if rcmd_press_time.get().is_some() && !is_modifier_key_code(key_code) && key_code != RIGHT_COMMAND_KEY_CODE {
                    rcmd_press_time.set(None);
                }
            }
            _ => {}
        }
        Some(event.clone())
    };

    let tap_result = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::KeyDown, CGEventType::KeyUp, CGEventType::FlagsChanged],
        callback_closure,
    );

    match tap_result {
        Ok(tap) => {
            unsafe {
                let mach_port_ref = tap.mach_port.as_concrete_TypeRef();
                let source = CFMachPortCreateRunLoopSource(ptr::null_mut(), mach_port_ref, 0);
                if source.is_null() {
                    eprintln!("Failed to create run loop source");
                    return;
                }
                let cf_run_loop_source = CFRunLoopSource::wrap_under_get_rule(source);
                current_run_loop.add_source(&cf_run_loop_source, kCFRunLoopCommonModes);
                tap.enable();
            }
            println!("Event tap enabled");
            CFRunLoop::run_current();
            println!("Event loop exited");
        }
        Err(e) => {
            eprintln!("Failed to create event tap: {:?}", e);
        }
    }
}

fn is_modifier_key_code(key_code: i64) -> bool {
    matches!(key_code, 54 | 55 | 56 | 57 | 58 | 59 | 60 | 61 | 62 | 63)
}

fn main() -> Result<(), String> { 
    println!("Starting mouseless");

    let (event_tx, event_rx): (Sender<GlobalEvent>, Receiver<GlobalEvent>) = channel();
    let eframe_control = EframeControl::default();
    let lshift_key_is_pressed_arc = Arc::new(AtomicBool::new(false));

    let listener_shared_state = EventTapSharedState {
        event_tx: event_tx.clone(),
        app_is_visible: eframe_control.is_visible.clone(),
        eframe_hide_requested_by_listener: eframe_control.hide_requested.clone(),
        lshift_key_is_pressed: lshift_key_is_pressed_arc.clone(),
    };
    thread::spawn(move || {
        global_event_listener_thread(listener_shared_state);
    });
    println!("Global event listener spawned");

    let placeholder_initial_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0,100.0));

    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_maximized(true) 
            .with_visible(false)
            .with_title("Mouseless Overlay"),
        ..Default::default()
    };
    
    println!("Starting eframe app (initially hidden)");
    let eframe_control_clone_for_app = eframe_control.clone();
    let lshift_arc_clone_for_app = lshift_key_is_pressed_arc.clone();
    let run_result = eframe::run_native(
        "Mouseless",
        native_options,
        Box::new(move |cc| {
            unsafe {
                let ns_app_class = class!(NSApplication);
                let ns_app: *mut Object = msg_send![ns_app_class, sharedApplication];
                let _: () = msg_send![ns_app, setActivationPolicy: cocoa::appkit::NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory];
                println!("Set app as accessory (won't appear in dock)");
            }
            Box::new(MouselessApp::new(cc, eframe_control_clone_for_app, placeholder_initial_rect, event_rx, lshift_arc_clone_for_app))
        }),
    );

    if let Err(e) = run_result {
        eprintln!("App error: {:?}", e);
        return Err(format!("Eframe error: {:?}", e));
    }

    println!("App exited successfully");
    Ok(())
}
