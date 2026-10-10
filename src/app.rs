use crate::renderer::Renderer;
use std::{
    error::Error,
    sync::{Arc, mpsc::Receiver},
};
use ttyrantula::{
    clipboard::SystemClipboard,
    input::{
        ApplicationAction, HistoryDirection, InputContext, Key as InputKey, KeyChord,
        KeySequenceState, KeymapResult, Modifiers, adjacent_command, resolve_key_sequence,
    },
    model::{CommandId, CopyScope, Session},
    pty::Size,
    session::{SnapshotMailbox, TerminalEvent, TerminalHandle, spawn_worker},
    terminal::Snapshot,
    ui::{
        CommandAction, InputMode, Point, UiFocus, UiState, append_terminal_snapshot, build_scene,
        hit_test_action, hit_test_command, hit_test_section, scroll_to_reveal_command,
    },
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{CursorIcon, Window, WindowId},
};

pub fn run() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let mut application = Application::new(event_loop.create_proxy());
    event_loop.run_app(&mut application)?;
    Ok(())
}

enum UserEvent {
    ClipboardText(Result<String, String>),
}

struct Application {
    session: Session,
    ui: UiState,
    clipboard: Option<SystemClipboard>,
    renderer: Option<Renderer>,
    cursor_position: Point,
    modifiers: ModifiersState,
    key_sequence: KeySequenceState,
    event_proxy: EventLoopProxy<UserEvent>,
    terminal_handle: Option<TerminalHandle>,
    terminal_snapshots: Option<SnapshotMailbox>,
    terminal_events: Option<Receiver<TerminalEvent>>,
    terminal_snapshot: Option<Snapshot>,
}

impl Application {
    fn new(event_proxy: EventLoopProxy<UserEvent>) -> Self {
        let session = Session::prototype();
        let ui = UiState::for_session(&session);
        let terminal = Size::new(80, 24)
            .ok()
            .and_then(|size| match spawn_worker("sh", size) {
                Ok((handle, snapshots, events)) => Some((handle, snapshots, events)),
                Err(error) => {
                    eprintln!("failed to start terminal worker: {error}");
                    None
                }
            });

        Self {
            session,
            ui,
            clipboard: None,
            renderer: None,
            cursor_position: Point::default(),
            modifiers: ModifiersState::default(),
            key_sequence: KeySequenceState::default(),
            event_proxy,
            terminal_handle: terminal.as_ref().map(|(handle, _, _)| handle.clone()),
            terminal_snapshots: terminal.as_ref().map(|(_, snapshots, _)| snapshots.clone()),
            terminal_events: terminal.map(|(_, _, events)| events),
            terminal_snapshot: None,
        }
    }

    fn scene(&self) -> Option<ttyrantula::ui::Scene> {
        self.renderer.as_ref().map(|renderer| {
            let size = renderer.window().inner_size();
            let mut scene = build_scene(
                &self.session,
                &self.ui,
                size.width,
                size.height,
                renderer.window().scale_factor() as f32,
            );
            if let Some(snapshot) = &self.terminal_snapshot {
                append_terminal_snapshot(&mut scene, snapshot);
            }
            scene
        })
    }

    fn poll_terminal(&mut self) {
        if let Some(mailbox) = &self.terminal_snapshots
            && let Some(snapshot) = mailbox.take_latest()
        {
            self.terminal_snapshot = Some(snapshot);
            self.request_redraw();
        }

        let Some(events) = &self.terminal_events else {
            return;
        };
        while let Ok(event) = events.try_recv() {
            match event {
                TerminalEvent::ChildExited(status) => {
                    eprintln!("terminal child exited with {:?}", status.code);
                }
                TerminalEvent::Error(error) => eprintln!("terminal worker failed: {error}"),
            }
        }
    }

    fn resize_terminal(&self, size: PhysicalSize<u32>) {
        let Some(handle) = &self.terminal_handle else {
            return;
        };
        let columns = (size.width / 8).clamp(1, u16::MAX as u32) as u16;
        let rows = (size.height / 18).clamp(1, u16::MAX as u32) as u16;
        let terminal_size = Size {
            columns,
            rows,
            pixel_width: size.width.min(u16::MAX as u32) as u16,
            pixel_height: size.height.min(u16::MAX as u32) as u16,
        };
        if let Err(error) = handle.resize(terminal_size) {
            eprintln!("terminal resize failed: {error}");
        }
    }

    fn update_hover(&mut self) {
        let Some(scene) = self.scene() else {
            return;
        };
        let hovered_command = hit_test_command(&scene, self.cursor_position);
        let hovered_section = hit_test_section(&scene, self.cursor_position);
        let hovered_target = hit_test_action(&scene, self.cursor_position);
        let hovered_action = hovered_target.map(|target| (target.id, target.action));

        if self.ui.hovered_command != hovered_command
            || self.ui.hovered_section != hovered_section
            || self.ui.hovered_action != hovered_action
            || self.ui.input_mode != InputMode::Pointer
        {
            self.key_sequence = KeySequenceState::default();
            self.ui = self
                .ui
                .with_hover(hovered_command, hovered_section, hovered_action);
            if let Some(renderer) = &self.renderer {
                renderer.window().set_cursor(
                    if hovered_target.is_some_and(|target| target.enabled) {
                        CursorIcon::Pointer
                    } else {
                        CursorIcon::Default
                    },
                );
                renderer.window().request_redraw();
            }
        }
    }

    fn activate_command_action_at_cursor(&mut self) {
        let Some(target) = self
            .scene()
            .as_ref()
            .and_then(|scene| hit_test_action(scene, self.cursor_position))
        else {
            return;
        };
        if !target.enabled {
            return;
        }

        self.perform_command_action(target.id, target.action);
        self.update_hover();
        self.request_redraw();
    }

    fn perform_command_action(&mut self, command_id: CommandId, action: CommandAction) {
        match action {
            CommandAction::ToggleExpansion => {
                self.ui = self.ui.with_toggled_expansion(command_id);
                self.ensure_command_visible(command_id);
            }
            CommandAction::Copy(scope) => self.copy_command(command_id, scope),
            CommandAction::Rerun => {}
        }
    }

    fn copy_command(&self, command_id: CommandId, scope: CopyScope) {
        let text = self
            .session
            .commands
            .iter()
            .find(|command| command.id == command_id)
            .map(|command| command.clipboard_text(scope));

        if let (Some(text), Some(clipboard)) = (text, &self.clipboard)
            && let Err(error) = clipboard.copy(text)
        {
            eprintln!("clipboard copy failed: {error}");
        }
    }

    fn request_paste(&self) {
        let Some(clipboard) = &self.clipboard else {
            return;
        };
        let event_proxy = self.event_proxy.clone();
        let requested = clipboard.request_text(move |result| {
            let event = UserEvent::ClipboardText(result.map_err(|error| error.to_string()));
            if event_proxy.send_event(event).is_err() {
                eprintln!("clipboard paste completed after the event loop closed");
            }
        });
        if let Err(error) = requested {
            eprintln!("clipboard paste failed: {error}");
        }
    }

    fn input_context(&self) -> InputContext {
        match self.ui.focus {
            UiFocus::Composer => InputContext::Composer,
            UiFocus::History(_) => InputContext::History,
        }
    }

    fn handle_keyboard_input(&mut self, event: KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }

        if let Some(bytes) = terminal_input_bytes(&event, self.modifiers)
            && let Some(handle) = &self.terminal_handle
        {
            if let Err(error) = handle.input(bytes) {
                eprintln!("terminal input failed: {error}");
            }
            return;
        }

        if event.repeat && self.key_sequence.is_pending() {
            return;
        }

        let keymap_result = event
            .text
            .as_deref()
            .and_then(normalized_character)
            .or_else(|| match &event.logical_key.as_ref() {
                Key::Character(value) => normalized_character(value),
                Key::Named(NamedKey::ArrowUp) => Some(InputKey::ArrowUp),
                Key::Named(NamedKey::ArrowDown) => Some(InputKey::ArrowDown),
                Key::Named(NamedKey::Space) => Some(InputKey::Space),
                Key::Named(NamedKey::Enter) => Some(InputKey::Enter),
                Key::Named(NamedKey::Escape) => Some(InputKey::Escape),
                _ => None,
            })
            .map(|key| KeyChord {
                key,
                modifiers: Modifiers {
                    control: self.modifiers.control_key(),
                    alt: self.modifiers.alt_key(),
                    shift: self.modifiers.shift_key(),
                    super_key: self.modifiers.super_key(),
                },
            })
            .map(|chord| resolve_key_sequence(self.input_context(), self.key_sequence, chord));

        if let Some((key_sequence, result)) = keymap_result {
            self.key_sequence = key_sequence;
            match result {
                KeymapResult::Pending => {
                    self.ui = self.ui.with_keyboard_focus(self.ui.focus);
                    self.request_redraw();
                    return;
                }
                KeymapResult::Action(action) => {
                    if !event.repeat || !matches!(action, ApplicationAction::Command(_)) {
                        self.apply_application_action(action);
                    }
                    return;
                }
                KeymapResult::Unmatched => {}
            }
        } else {
            self.key_sequence = KeySequenceState::default();
        }

        if self.ui.focus != UiFocus::Composer {
            return;
        }

        self.ui = self.ui.with_keyboard_focus(UiFocus::Composer);
        self.session = if event.logical_key == Key::Named(NamedKey::Backspace) {
            self.session.with_composer_backspace()
        } else {
            event.text.as_deref().map_or_else(
                || self.session.clone(),
                |text| self.session.with_composer_text(text),
            )
        };
        self.request_redraw();
    }

    fn apply_application_action(&mut self, action: ApplicationAction) {
        match action {
            ApplicationAction::FocusComposer => {
                self.ui = self.ui.with_keyboard_focus(UiFocus::Composer);
            }
            ApplicationAction::FocusHistory => {
                if let Some(command_id) = self.session.commands.last().map(|command| command.id) {
                    self.focus_history_command(command_id);
                }
            }
            ApplicationAction::Paste => self.request_paste(),
            ApplicationAction::MoveHistoryFocus(direction) => {
                self.move_history_focus(direction);
            }
            ApplicationAction::Command(action) => {
                if let Some(command_id) = self.ui.focused_command() {
                    self.ui = self.ui.with_keyboard_focus(UiFocus::History(command_id));
                    self.perform_command_action(command_id, action);
                }
            }
        }
        self.request_redraw();
    }

    fn move_history_focus(&mut self, direction: HistoryDirection) {
        let Some(current) = self.ui.focused_command() else {
            return;
        };
        if let Some(command_id) = adjacent_command(&self.session.commands, current, direction) {
            self.focus_history_command(command_id);
        }
    }

    fn focus_history_command(&mut self, command_id: CommandId) {
        self.ui = self.ui.with_keyboard_focus(UiFocus::History(command_id));
        self.ensure_command_visible(command_id);
    }

    fn ensure_command_visible(&mut self, command_id: CommandId) {
        let Some(scene) = self.scene() else {
            return;
        };
        let scroll = scroll_to_reveal_command(&scene, self.ui.history_scroll, command_id);
        self.ui = self
            .ui
            .with_history_scroll(scroll, scene.history_scroll_limit);
    }

    fn scroll_history(&mut self, delta: MouseScrollDelta) {
        let Some(scene) = self.scene() else {
            return;
        };
        if !scene.history_bounds.contains(self.cursor_position) {
            return;
        }

        let scale_factor = self
            .renderer
            .as_ref()
            .map_or(1.0, |renderer| renderer.window().scale_factor() as f32);
        let pixels = match delta {
            MouseScrollDelta::LineDelta(_, vertical) => vertical * 48.0 * scale_factor,
            MouseScrollDelta::PixelDelta(position) => position.y as f32,
        };
        self.ui = self
            .ui
            .with_scroll_delta(pixels, scene.history_scroll_limit);
        self.update_hover();
        self.request_redraw();
    }

    fn request_redraw(&self) {
        if let Some(renderer) = &self.renderer {
            renderer.window().request_redraw();
        }
    }
}

impl ApplicationHandler<UserEvent> for Application {
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.poll_terminal();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("ttyrantula")
            .with_inner_size(LogicalSize::new(1040.0, 760.0))
            .with_min_inner_size(LogicalSize::new(520.0, 420.0));

        let initialized = event_loop
            .create_window(attributes)
            .map(Arc::new)
            .map_err(|error| Box::new(error) as Box<dyn Error>)
            .and_then(|window| {
                let clipboard = SystemClipboard::new(window.clone())?;
                pollster::block_on(Renderer::new(window, event_loop))
                    .map(|renderer| (renderer, clipboard))
            });

        match initialized {
            Ok((renderer, clipboard)) => {
                self.resize_terminal(renderer.window().inner_size());
                renderer.window().request_redraw();
                self.clipboard = Some(clipboard);
                self.renderer = Some(renderer);
            }
            Err(error) => {
                eprintln!("failed to initialize ttyrantula: {error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self
            .renderer
            .as_ref()
            .is_none_or(|renderer| renderer.window().id() != window_id)
        {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size);
                    renderer.window().request_redraw();
                }
                self.resize_terminal(size);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(renderer) = &self.renderer {
                    self.resize_terminal(renderer.window().inner_size());
                    renderer.window().request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Point {
                    x: position.x as f32,
                    y: position.y as f32,
                };
                self.update_hover();
            }
            WindowEvent::CursorLeft { .. } => {
                self.key_sequence = KeySequenceState::default();
                self.ui = self.ui.with_hover(None, None, None);
                if let Some(renderer) = &self.renderer {
                    renderer.window().set_cursor(CursorIcon::Default);
                    renderer.window().request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.activate_command_action_at_cursor(),
            WindowEvent::MouseWheel { delta, .. } => self.scroll_history(delta),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => self.handle_keyboard_input(event),
            WindowEvent::Focused(false) => {
                self.modifiers = ModifiersState::default();
                self.key_sequence = KeySequenceState::default();
            }
            WindowEvent::RedrawRequested => {
                let Some(scene) = self.scene() else {
                    return;
                };
                let Some(renderer) = &mut self.renderer else {
                    return;
                };

                if let Err(error) = renderer.render(&scene) {
                    eprintln!("rendering failed: {error}");
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::ClipboardText(Ok(text)) => {
                self.session = self.session.with_composer_text(&text);
                self.request_redraw();
            }
            UserEvent::ClipboardText(Err(error)) => {
                eprintln!("clipboard paste failed: {error}");
            }
        }
    }
}

fn normalized_character(value: &str) -> Option<InputKey> {
    if value == " " {
        return Some(InputKey::Space);
    }

    let mut characters = value.chars();
    let character = characters.next()?;
    (characters.next().is_none() && !character.is_control())
        .then_some(InputKey::Character(character.to_ascii_lowercase()))
}

fn terminal_input_bytes(event: &KeyEvent, modifiers: ModifiersState) -> Option<Vec<u8>> {
    let mut bytes = if modifiers.control_key() {
        match &event.logical_key {
            Key::Character(value) => value
                .chars()
                .next()
                .map(|character| vec![character.to_ascii_lowercase() as u8 & 0x1f]),
            _ => None,
        }
    } else if let Some(text) = event.text.as_deref().filter(|text| !text.is_empty()) {
        Some(text.as_bytes().to_vec())
    } else {
        match event.logical_key {
            Key::Named(NamedKey::Enter) => Some(vec![b'\r']),
            Key::Named(NamedKey::Backspace) => Some(vec![0x7f]),
            Key::Named(NamedKey::Tab) => Some(vec![b'\t']),
            Key::Named(NamedKey::Escape) => Some(vec![0x1b]),
            Key::Named(NamedKey::ArrowUp) => Some(b"\x1b[A".to_vec()),
            Key::Named(NamedKey::ArrowDown) => Some(b"\x1b[B".to_vec()),
            Key::Named(NamedKey::ArrowRight) => Some(b"\x1b[C".to_vec()),
            Key::Named(NamedKey::ArrowLeft) => Some(b"\x1b[D".to_vec()),
            Key::Named(NamedKey::Home) => Some(b"\x1b[H".to_vec()),
            Key::Named(NamedKey::End) => Some(b"\x1b[F".to_vec()),
            Key::Named(NamedKey::Delete) => Some(b"\x1b[3~".to_vec()),
            _ => None,
        }
    }?;

    if modifiers.alt_key() {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}
