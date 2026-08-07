use std::io;
use crossterm::event::{self,Event,KeyCode,KeyEvent,KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Layout, Direction, Constraint, Alignment},
    widgets::{Widget, Block, Borders, Paragraph, Clear},
    style::{Style, Stylize, Color},
    text::Line,
    DefaultTerminal,Frame,
};
mod list;
mod map;
mod monitor;
mod rotation;
mod resolutions;
mod utils;
mod scale;
mod configuration;
mod test_utils;

use list::MonitorList;
use map::Map;
use monitor::Monitor;

use resolutions::Resolutions; 
use scale::Scale;
use utils::TUIMode;
use configuration::Configuration;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::default().run(&mut terminal);
    ratatui::restore();
    app_result
}

#[derive(Debug, Default)]
struct App {
    exit:bool,
    config: Configuration,
    monitors: Vec<Monitor>,
    selected_monitor: usize,
    selected_resolution : usize,
    selected_scale: usize,
    mode: TUIMode,
    show_help: bool,
    show_popup: Option<Popup>,
}

#[derive(Debug)]
struct Popup {
    title: String,
    lines: Vec<String>,
    is_error: bool,
}

impl App{
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        self.monitors = Monitor::get_monitors();
        
        // Load saved monitor positions/scales
        if let Some(saved_states) = Configuration::load_monitor_state() {
            for monitor in &mut self.monitors {
                if let Some(saved_state) = saved_states.iter().find(|s| s.name == monitor.name) {
                    if let Some(pos) = &saved_state.position {
                        monitor.position = Some(pos.clone());
                    }
                    if let Some(scale) = saved_state.scale {
                        monitor.scale = Some(scale);
                    }
                    if let Some(workspace) = saved_state.workspace {
                        monitor.workspace = Some(workspace);
                    }
                }
            }
        }
        
        self.selected_resolution= 0;
        self.selected_monitor= 0;
        self.config = Configuration::get();

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame){
        frame.render_widget(self,frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.show_help {
            match key_event.code {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('K') | KeyCode::Char('k') => self.show_help = false,
                _ => {}
            }
            return;
        }

        if self.show_popup.is_some() {
            match key_event.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char(' ') => self.show_popup = None,
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    self.show_popup = None;
                    self.write();
                }
                _ => {}
            }
            return;
        }

        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Char('w') => {
                match self.validate() {
                    Ok(_) => self.write(),
                    Err(errs) => self.show_popup = Some(Popup {
                        title: " Error ".to_string(),
                        lines: errs,
                        is_error: true,
                    }),
                }
            }, 
            KeyCode::Char('K') if self.mode != TUIMode::Move => self.show_help = true,
            _ => {
                match self.mode {
                    TUIMode::View => MonitorList::handle_events(self,key_event),
                    TUIMode::Move => Map::handle_events(self,key_event),
                    TUIMode::Resolution=> Resolutions::handle_events(self,key_event),
                    TUIMode::Scale => Scale::handle_events(self,key_event), 
                }
            }
        }
    }
    
    fn exit(&mut self) {
        // Save monitor state before exiting
        if let Err(e) = Configuration::save_monitor_state(&self.monitors) {
            eprintln!("Warning: Failed to save monitor state on exit: {}", e);
        }
        self.exit = true;
    }
    
    fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        // Duplicate workspace check
        let mut ws_counts = std::collections::HashMap::new();
        for m in &self.monitors {
            if let Some(ws) = m.workspace {
                *ws_counts.entry(ws).or_insert(0) += 1;
            }
        }
        
        let mut duplicated_ws = Vec::new();
        for (ws, count) in ws_counts {
            if count > 1 {
                duplicated_ws.push(ws.to_string());
            }
        }
        if !duplicated_ws.is_empty() {
            errors.push(format!("Duplicate workspace assignment detected: {}", duplicated_ws.join(", ")));
        }

        // Contiguous check
        let enabled_indices: Vec<usize> = self.monitors.iter().enumerate().filter(|(_, m)| m.enabled).map(|(i, _)| i).collect();
        
        if enabled_indices.len() > 1 {
            let mut adj = vec![vec![]; enabled_indices.len()];
            let mut geoms = Vec::new();
            for &idx in &enabled_indices {
                geoms.push(self.monitors[idx].get_geometry());
            }
            
            let eps = 2.0;
            for i in 0..geoms.len() {
                for j in (i+1)..geoms.len() {
                    let (x1, y1, w1, h1) = geoms[i];
                    let (x2, y2, w2, h2) = geoms[j];
                    
                    // Overlap check
                    if x1 < x2 + w2 && x2 < x1 + w1 && y1 < y2 + h2 && y2 < y1 + h1 {
                        let name1 = &self.monitors[enabled_indices[i]].name;
                        let name2 = &self.monitors[enabled_indices[j]].name;
                        errors.push(format!("Monitors overlap: {} and {}", name1, name2));
                    }
                    
                    let touches_x = x1 <= x2 + w2 + eps && x2 <= x1 + w1 + eps;
                    let touches_y = y1 <= y2 + h2 + eps && y2 <= y1 + h1 + eps;
                    
                    if touches_x && touches_y {
                        adj[i].push(j);
                        adj[j].push(i);
                    }
                }
            }
            
            let mut components = Vec::new();
            let mut global_visited = vec![false; enabled_indices.len()];
            
            for i in 0..enabled_indices.len() {
                if !global_visited[i] {
                    let mut comp = Vec::new();
                    let mut q = vec![i];
                    global_visited[i] = true;
                    
                    while let Some(node) = q.pop() {
                        comp.push(node);
                        for &neighbor in &adj[node] {
                            if !global_visited[neighbor] {
                                global_visited[neighbor] = true;
                                q.push(neighbor);
                            }
                        }
                    }
                    components.push(comp);
                }
            }
            
            if components.len() > 1 {
                components.sort_by(|a, b| b.len().cmp(&a.len()));
                let mut disconnected = Vec::new();
                for comp in components.iter().skip(1) {
                    for &idx in comp {
                        disconnected.push(self.monitors[enabled_indices[idx]].name.clone());
                    }
                }
                errors.push(format!("Monitors not contiguous. Disconnected: {}", disconnected.join(", ")));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    fn write(&mut self) {
        let path = self.config.monitors_config_path.as_deref().unwrap_or("~/.config/hypr/monitors.conf");
        let lua_config = self.config.lua_monitor_config.as_deref();

        if Monitor::save_hyprland_config(path, &self.monitors, lua_config).is_err() {
            self.show_popup = Some(Popup {
                title: " Error ".to_string(),
                lines: vec!["Failed to save Hyprland config.".to_string()],
                is_error: true,
            });
        } else {
            let _ = std::process::Command::new("hyprctl")
                .arg("reload")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }

        if let Err(e) = Configuration::save_monitor_state(&self.monitors) {
            eprintln!("✗ Failed to save monitor state: {}", e);
        }
    }         

}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

impl Widget for &App {

    fn render(self,area: Rect, buf: &mut Buffer) {
        let mut monitor_list = MonitorList::new(
            &self.monitors,
            self.mode,
            Some(self.selected_monitor), 
        );

        let canvas = Map {
            mode: self.mode,
            selected: self.selected_monitor,
            monitors: &self.monitors,
        };
        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(70),
                Constraint::Percentage(30),
            ])
            .split(area);

        match self.mode {
            TUIMode::Resolution=> {
                let selected = &self.monitors[self.selected_monitor];
                let mut resolutions = Resolutions::new(
                        selected,
                        Some(self.selected_resolution)
                );    
                let inner_top_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(vec![
                        Constraint::Percentage(70),
                        Constraint::Percentage(30),
                    ])
                    .split(outer_layout[0]);
                canvas.render(inner_top_layout[0], buf);
                resolutions.render(inner_top_layout[1], buf);
            }
            TUIMode::Scale => {
                let mut scale = Scale::new(self.selected_scale);
                let inner_top_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(vec![
                        Constraint::Percentage(90),
                        Constraint::Percentage(10),
                    ])
                    .split(outer_layout[0]);
                canvas.render(inner_top_layout[0], buf);
                scale.render(inner_top_layout[1], buf);
            }
            _ => {
                canvas.render(outer_layout[0], buf);
            }
        }
        monitor_list.render(outer_layout[1], buf);

        if self.show_help {
            let popup_area = centered_rect(60, 60, area);
            let text = vec![
                Line::from(" --- Global --- ".bold().yellow()),
                Line::from("Save <w> | Quit <q> | Close Help <K/Esc>"),
                Line::from(""),
                Line::from(" --- View Mode --- ".bold().yellow()),
                Line::from("Up <k> | Down <j>"),
                Line::from("Move <m> | Resolution <r> | Scale <s>"),
                Line::from("Rotate <o> | Workspace <0-9>"),
                Line::from("Enable <e> | Disable <d>"),
                Line::from(""),
                Line::from(" --- Move Mode --- ".bold().yellow()),
                Line::from("Fast <Shift+*> | Up <k> | Down <j>"),
                Line::from("Left <h> | Right <l> | Quit <Esc>"),
                Line::from(""),
                Line::from(" --- Res / Scale Mode --- ".bold().yellow()),
                Line::from("Select <Space> | Quit <Esc> | Up/Down <k/j>"),
            ];

            let p = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title(" Keybindings ".bold().white()).border_style(Style::default().fg(Color::Cyan)))
                .alignment(Alignment::Center);

            Clear.render(popup_area, buf);
            p.render(popup_area, buf);
        }

        if let Some(ref popup) = self.show_popup {
            let popup_area = centered_rect(50, 40, area);
            let mut text = vec![
                Line::from(""),
            ];
            for line in &popup.lines {
                text.push(Line::from(line.clone()));
                text.push(Line::from(""));
            }
           if popup.is_error {
                text.push(Line::from("Press <f> to force write anyway, or <Esc>, <Enter>, <q> to close.".gray()));
            } 
            let color = if popup.is_error { Color::Red } else { Color::Yellow };
            let border_style = Style::default().fg(color);
            let title = Line::from(popup.title.clone().bold().white());

            let p = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title(title).border_style(border_style))
                .alignment(Alignment::Center);

            Clear.render(popup_area, buf);
            p.render(popup_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::test_monitors;
    use crossterm::event::KeyModifiers;
   
    #[test]
    fn handle_mode_view_key_event() -> io::Result<()> {
        let mut app = App{
            monitors: test_monitors(),
            selected_monitor: 0,
            ..Default::default()
        };

        app.handle_key_event(KeyCode::Char('k').into());
        assert_eq!(app.selected_monitor, 1);

        app.handle_key_event(KeyCode::Char('j').into());
        assert_eq!(app.selected_monitor, 0);

        app.handle_key_event(KeyCode::Char('j').into());
        assert_eq!(app.selected_monitor, app.monitors.len() - 1);

        app.handle_key_event(KeyCode::Char('k').into());
        assert_eq!(app.selected_monitor, 0);
       
        app.handle_key_event(KeyCode::Char('m').into());
        assert_eq!(app.mode, TUIMode::Move);
    
        app.handle_key_event(KeyCode::Esc.into());
        assert_eq!(app.mode, TUIMode::View);

        app.handle_key_event(KeyCode::Char('r').into());
        assert_eq!(app.mode, TUIMode::Resolution);
    
        app.handle_key_event(KeyCode::Esc.into());
        assert_eq!(app.mode, TUIMode::View);

        app.handle_key_event(KeyCode::Char('s').into());
        assert_eq!(app.mode, TUIMode::Scale);
    
        app.handle_key_event(KeyCode::Esc.into());
        assert_eq!(app.mode, TUIMode::View);

        app.handle_key_event(KeyCode::Char('q').into());
        assert!(app.exit);

        Ok(())
    }
     
         
    #[test]
    fn handle_mode_move_key_event() -> io::Result<()> {
        let mut app = App{
            monitors: test_monitors(),
            selected_monitor: 0,
            ..Default::default()
        };
 
        app.handle_key_event(KeyCode::Char('m').into());
        assert_eq!(app.mode, TUIMode::Move);

        // K (Shift+k) moves -10
        app.handle_key_event(KeyCode::Char('K').into());
        let monitor = app.monitors[app.selected_monitor].clone();
        assert_eq!(monitor.position.unwrap().y, -10);

        // J (Shift+j) moves +10
        app.handle_key_event(KeyCode::Char('J').into());
        let monitor = app.monitors[app.selected_monitor].clone();
        assert_eq!(monitor.position.unwrap().y, 0);

        // H moves -10
        app.handle_key_event(KeyCode::Char('H').into());
        let monitor = app.monitors[app.selected_monitor].clone();
        assert_eq!(monitor.position.unwrap().x, -10);

        // L moves +10
        app.handle_key_event(KeyCode::Char('L').into());
        let monitor = app.monitors[app.selected_monitor].clone();
        assert_eq!(monitor.position.unwrap().x, 0);

        app.handle_key_event(KeyCode::Char('q').into());
        assert!(app.exit);

        Ok(())
    }       
    #[test]
    fn handle_mode_resolution_key_event() -> io::Result<()> {
        let mut app = App{
            monitors: test_monitors(),
            selected_monitor: 0,
            ..Default::default()
        };

        app.handle_key_event(KeyCode::Char('r').into());
        assert_eq!(app.mode, TUIMode::Resolution);

        app.selected_resolution = 0;
        app.handle_key_event(KeyCode::Char('j').into());
        assert_eq!(app.selected_resolution, 1);

        app.handle_key_event(KeyCode::Char('k').into());
        assert_eq!(app.selected_resolution, 0);

        app.handle_key_event(KeyCode::Char(' ').into());
        let monitor = app.monitors[0].clone();
        assert_eq!(monitor.modes[0].current, true);

        app.handle_key_event(KeyCode::Char('q').into());
        assert!(app.exit);

        Ok(())
    }    

    #[test]
    fn handle_mode_scale_key_event() -> io::Result<()> {
        let mut app = App{
            monitors: test_monitors(),
            selected_monitor: 0,
            ..Default::default()
        };

        app.handle_key_event(KeyCode::Char('s').into());
        assert_eq!(app.mode, TUIMode::Scale);

        app.selected_scale = 0;
        app.handle_key_event(KeyCode::Char('j').into());
        assert_eq!(app.selected_scale, 1);

        app.handle_key_event(KeyCode::Char('k').into());
        assert_eq!(app.selected_scale, 0);

        app.handle_key_event(KeyCode::Char(' ').into());
        let monitor = app.monitors[0].clone();
        assert_eq!(monitor.scale, Some(0.5));

        app.handle_key_event(KeyCode::Char('q').into());
        assert!(app.exit);

        Ok(())
    }       
    #[test]
    fn handle_mode_view_arrow_key_event() -> io::Result<()> {
        let mut app = App{
            monitors: test_monitors(),
            selected_monitor: 0,
            ..Default::default()
        };

        app.handle_key_event(KeyCode::Up.into());
        assert_eq!(app.selected_monitor, 1);

        app.handle_key_event(KeyCode::Down.into());
        assert_eq!(app.selected_monitor, 0);

        app.handle_key_event(KeyCode::Down.into());
        assert_eq!(app.selected_monitor, app.monitors.len() - 1);

        app.handle_key_event(KeyCode::Up.into());
        assert_eq!(app.selected_monitor, 0);
        
        Ok(())
    }

    #[test]
    fn handle_mode_move_arrow_key_event() -> io::Result<()> {
        let mut app = App{
            monitors: test_monitors(),
            selected_monitor: 0,
            ..Default::default()
        };
 
        app.handle_key_event(KeyCode::Char('m').into());
        assert_eq!(app.mode, TUIMode::Move);

        // Shift+Up moves -10
        app.handle_key_event(KeyEvent{
            code: KeyCode::Up,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: event::KeyEventState::empty(),
        });
        let monitor = app.monitors[app.selected_monitor].clone();
        assert_eq!(monitor.position.unwrap().y, -10);

        // Shift+Down moves +10
        app.handle_key_event(KeyEvent{
             code: KeyCode::Down,
             modifiers: KeyModifiers::SHIFT,
             kind: KeyEventKind::Press,
             state: event::KeyEventState::empty(),
        });
        let monitor = app.monitors[app.selected_monitor].clone();
        assert_eq!(monitor.position.unwrap().y, 0);

        // Shift+Left moves -10
        app.handle_key_event(KeyEvent{
             code: KeyCode::Left,
             modifiers: KeyModifiers::SHIFT,
             kind: KeyEventKind::Press,
             state: event::KeyEventState::empty(),
        });
        let monitor = app.monitors[app.selected_monitor].clone();
        assert_eq!(monitor.position.unwrap().x, -10);

        // Shift+Right moves +10
        app.handle_key_event(KeyEvent{
             code: KeyCode::Right,
             modifiers: KeyModifiers::SHIFT,
             kind: KeyEventKind::Press,
             state: event::KeyEventState::empty(),
        });
        let monitor = app.monitors[app.selected_monitor].clone();
        assert_eq!(monitor.position.unwrap().x, 0);


        // Snap Test
        // Move to 100
        app.monitors[app.selected_monitor].position.as_mut().unwrap().y = 100;
        
        // Up (no shift) should snap to 0 (which is a generic target for all monitors)
        app.handle_key_event(KeyCode::Up.into());
        let monitor = app.monitors[app.selected_monitor].clone();
        assert_eq!(monitor.position.unwrap().y, 0);

        Ok(())
    }

    #[test]
    fn handle_mode_resolution_arrow_key_event() -> io::Result<()> {
        let mut app = App{
            monitors: test_monitors(),
            selected_monitor: 0,
            ..Default::default()
        };

        app.handle_key_event(KeyCode::Char('r').into());
        assert_eq!(app.mode, TUIMode::Resolution);

        app.selected_resolution = 0;
        app.handle_key_event(KeyCode::Down.into());
        assert_eq!(app.selected_resolution, 1);

        app.handle_key_event(KeyCode::Up.into());
        assert_eq!(app.selected_resolution, 0);

        Ok(())
    }

    #[test]
    fn handle_mode_scale_arrow_key_event() -> io::Result<()> {
        let mut app = App{
            monitors: test_monitors(),
            selected_monitor: 0,
            ..Default::default()
        };

        app.handle_key_event(KeyCode::Char('s').into());
        assert_eq!(app.mode, TUIMode::Scale);

        app.selected_scale = 0;
        app.handle_key_event(KeyCode::Down.into());
        assert_eq!(app.selected_scale, 1);

        app.handle_key_event(KeyCode::Up.into());
        assert_eq!(app.selected_scale, 0);

        Ok(())
    }
}
