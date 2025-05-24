use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::{Instant, Duration};
use std::sync::mpsc::Receiver;

use eframe::{egui};
use core_graphics::event::{CGEventType, CGEventTapLocation, CGMouseButton, CGEvent};
use core_graphics::geometry::CGPoint;
use core_graphics::event_source::CGEventSourceStateID;
use mouse_rs::Mouse;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use objc::{msg_send, sel, sel_impl};
use objc::runtime::Object;

#[cfg(target_os = "macos")]
use cocoa::appkit::{NSWindowCollectionBehavior, NSWindowStyleMask};
#[cfg(target_os = "macos")]
const NSNONACTIVATING_PANEL_MASK: u64 = 1 << 7;

use crate::grid::{self, MAIN_GRID_COLS, MAIN_GRID_ROWS, SUB_GRID_COLS, SUB_GRID_ROWS};
use crate::event_handler::{GlobalEvent};

#[derive(Clone)]
pub struct EframeControl {
    pub hide_requested: Arc<AtomicBool>,
    pub is_visible: Arc<AtomicBool>,
}

impl Default for EframeControl {
    fn default() -> Self {
        Self {
            hide_requested: Arc::new(AtomicBool::new(false)),
            is_visible: Arc::new(AtomicBool::new(false)),
        }
    }
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

pub struct MouselessApp {
    display_mode: grid::DisplayMode,
    key_input_buffer: String,
    selected_main_cell_index: Option<usize>,
    main_grid_labels: Vec<String>,
    main_grid_rects: Vec<egui::Rect>,
    sub_grid_labels: Vec<String>,
    sub_grid_rects: Vec<egui::Rect>,
    last_layout_screen_rect: egui::Rect,
    mouse_handler: Mouse,
    eframe_control: EframeControl,
    _initial_target_rect: egui::Rect, // Kept for future use
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
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        eframe_control: EframeControl,
        initial_target_rect: egui::Rect,
        event_rx: Receiver<GlobalEvent>,
        lshift_key_is_pressed: Arc<AtomicBool>,
    ) -> Self {
        let (labels, _) = grid::generate_main_grid_layout(
            MAIN_GRID_COLS,
            MAIN_GRID_ROWS,
            egui::Rect::from_min_size(egui::Pos2::ZERO, initial_target_rect.size()),
        );
        
        let s = Self {
            display_mode: grid::DisplayMode::MainGrid,
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
                        self.display_mode = grid::DisplayMode::MainGrid;
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
                self.display_mode = grid::DisplayMode::MainGrid;
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
                    self.display_mode = grid::DisplayMode::MainGrid;
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

        let current_content_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, ctx.screen_rect().size());
        if self.main_grid_rects.is_empty() || self.last_layout_screen_rect != current_content_rect {
            println!("Recalculating layout");
            let (labels, rects) = grid::generate_main_grid_layout(MAIN_GRID_COLS, MAIN_GRID_ROWS, current_content_rect);
            self.main_grid_labels = labels;
            self.main_grid_rects = rects;
            self.last_layout_screen_rect = current_content_rect;

            if self.display_mode == grid::DisplayMode::SubGrid {
                 if let Some(main_idx) = self.selected_main_cell_index {
                    if main_idx < self.main_grid_rects.len() {
                        let selected_main_rect = self.main_grid_rects[main_idx];
                        let (sg_labels, sg_rects) = grid::generate_sub_grid_layout(selected_main_rect, SUB_GRID_COLS, SUB_GRID_ROWS);
                        self.sub_grid_labels = sg_labels;
                        self.sub_grid_rects = sg_rects;
                    } else { self.display_mode = grid::DisplayMode::MainGrid; } 
                 } else { self.display_mode = grid::DisplayMode::MainGrid; } 
            }
        }
        
        if self.display_mode == grid::DisplayMode::MainGrid {
            let events = ctx.input(|i| i.events.clone());
            for event in events {
                if let egui::Event::Key { key, pressed: true, .. } = event {
                    if let Some(char_code) = key_to_char(key, Default::default()) {
                        self.key_input_buffer.push(char_code);
                        if self.key_input_buffer.len() == 2 {
                            if let Some(index) = self.main_grid_labels.iter().position(|label| *label == self.key_input_buffer) {
                                self.selected_main_cell_index = Some(index);
                                self.display_mode = grid::DisplayMode::SubGrid;
                                self.key_input_buffer.clear();
                                 if let Some(main_idx) = self.selected_main_cell_index { 
                                    if main_idx < self.main_grid_rects.len() {
                                        let selected_main_rect = self.main_grid_rects[main_idx];
                                        let (sg_labels, sg_rects) = grid::generate_sub_grid_layout(selected_main_rect, SUB_GRID_COLS, SUB_GRID_ROWS);
                                        self.sub_grid_labels = sg_labels;
                                        self.sub_grid_rects = sg_rects;
                                    } else { self.display_mode = grid::DisplayMode::MainGrid;}
                                 } else { self.display_mode = grid::DisplayMode::MainGrid;}
                            } else { self.key_input_buffer.clear(); }
                        }
                    }
                }
            }
        } else if self.display_mode == grid::DisplayMode::SubGrid {
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
                let main_cell_bg_color = egui::Color32::from_rgba_unmultiplied(50, 50, 50, 120); 
                let line_stroke = egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 100)); 
                let text_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200); 

                if !self.main_grid_rects.is_empty() {
                    for (index, rect) in self.main_grid_rects.iter().enumerate() {
                        let bg_color = if self.display_mode == grid::DisplayMode::SubGrid && Some(index) != self.selected_main_cell_index {
                            egui::Color32::from_rgba_unmultiplied(30, 30, 30, 70) 
                        } else { main_cell_bg_color };
                        painter.rect_filled(*rect, 0.0, bg_color);
                        painter.rect_stroke(*rect, 0.0, line_stroke);
                        if index < self.main_grid_labels.len() {
                            let cell_center = rect.center();
                            let font_size = rect.height().min(rect.width()) * 0.4;
                            painter.text(cell_center, egui::Align2::CENTER_CENTER, &self.main_grid_labels[index], egui::FontId::proportional(font_size), text_color);
                        }
                    }
                } else if self.display_mode == grid::DisplayMode::MainGrid {
                     painter.text(ctx.screen_rect().center(), egui::Align2::CENTER_CENTER, "Waiting for layout...", egui::FontId::default(), text_color);
                }

                if self.display_mode == grid::DisplayMode::SubGrid {
                    if self.sub_grid_rects.is_empty() && self.selected_main_cell_index.is_some() {
                         if let Some(idx) = self.selected_main_cell_index {
                            if idx < self.main_grid_rects.len() {
                                 let selected_rect = self.main_grid_rects[idx];
                                 painter.text(selected_rect.center(), egui::Align2::CENTER_CENTER, "Waiting for sub-layout...", egui::FontId::proportional(selected_rect.height() * 0.15), egui::Color32::YELLOW);
                            }
                        }
                    } else {
                        let sub_cell_bg_color = egui::Color32::from_rgba_unmultiplied(70, 70, 20, 160); 
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