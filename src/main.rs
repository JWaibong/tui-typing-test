use chrono::prelude::*;
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use rand::{distributions::Alphanumeric, prelude::*};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, Tabs, Gauge,
    },
    Terminal,
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("error reading the DB file: {0}")]
    ReadDBError(#[from] io::Error),
    #[error("error parsing the DB file: {0}")]
    ParseDBError(#[from] serde_json::Error),
}

enum Event<I> {
    Input(I),
    Tick,
}

#[derive(Copy, Clone, Debug)]
enum MenuItem {
    Home,
    Game,
    GameOver,
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Home => 0,
            MenuItem::Game => 1,
            MenuItem::GameOver => 2,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode().expect("can run in raw mode");

    let (tx, rx) = mpsc::channel();
    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));
                // timeout = 200ms - time since last_tick or 0  
                // timeout is the maximum duration to wait for an event before sending a new tick
            if event::poll(timeout).expect("poll works") {
                if let CEvent::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).expect("can send events"); //if we read a keyboard input, send it across the channel
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now(); // send a tick across the channel if we timed out
                }
            }
        }  
    });

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let menu_titles = vec!["Start", "Quit"];
    let mut curr_input = String::from("");
    let mut start = false; 
    let mut countdown : usize  = 3;
    let mut active_menu_item = MenuItem::Home;
    let mut game_start_time: Option<Instant> = None;

    let game_words = vec!["the", "lazy", "dog", "jumped", "over", "the", "quick", "brown", "fox"];

    loop {
        terminal.draw(|rect| {
            let size = rect.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        Constraint::Length(3), // menu
                        Constraint::Min(2),  // content 
                        Constraint::Length(3), // footer
                    ]
                    .as_ref(),
                )
                .split(size);

            match active_menu_item {
                MenuItem::Home => {
                let menu = menu_titles
                    .iter()
                    .map(|t| {
                        let (first, rest) = t.split_at(1);
                        Spans::from(vec![
                            Span::styled(
                                first,
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::UNDERLINED),
                            ),
                            Span::styled(rest, Style::default().fg(Color::White)),
                        ])
                    })
                    .collect();
    
                let tabs = Tabs::new(menu)
                    .block(Block::default().title("Menu").borders(Borders::ALL))
                    .style(Style::default().fg(Color::White)) // color non-active items white 
                    .highlight_style(Style::default().fg(Color::Yellow)) // color active_menu_item yellow 
                    .divider(Span::raw("|"));
    
                    rect.render_widget(tabs, chunks[0]);
                    rect.render_widget(render_home(start, &mut countdown), chunks[1]);
                    if countdown == 0 {
                        active_menu_item = MenuItem::Game;
                        game_start_time = Some(Instant::now());
                    }
                },
                MenuItem::Game => {
                    let text = Spans::from(vec![Span::raw(curr_input.as_str())]);
                    let input = Paragraph::new(text).block(Block::default().title("Input").borders(Borders::ALL));

                    rect.render_widget(input, chunks[0]);
                    
                    let in_game_timer = game_start_time.unwrap().elapsed().as_secs();

                    let mut percent = 100 - ((100.0 * in_game_timer as f32) / 60.0) as i32;
                    if percent < 0 {
                        percent = 0;
                    }

                    let progress_bar = Gauge::default()
                        .block(Block::default()
                        .borders(Borders::ALL)
                        .title("Time Remaining"))
                        .gauge_style(Style::default().fg(Color::White).bg(Color::Black))
                        .percent(percent as u16);
                    
                    rect.render_widget(progress_bar, chunks[2]);

                    
                },
                MenuItem::GameOver => {

                }
            };
        })?;

        if start {
            match rx.recv()? {
                Event::Input(event) => match event.code {
                    KeyCode::Backspace => {
                        curr_input.pop();
                    },
                    KeyCode::Char(c) => {
                        curr_input.push(c);
                    },
                    _ => {}
                },
                Event::Tick => {}
            }
        }
        else {
            match rx.recv()? {
                Event::Input(event) => match event.code {
                    KeyCode::Char('q') => {
                        disable_raw_mode()?;
                        terminal.show_cursor()?;
                        break;
                    }
                    KeyCode::Char('s') => start = true,
                    _ => {}
                },
                Event::Tick => {}
            }
        }
    }

    Ok(())
}

fn render_home<'a>(start: bool, countdown: &mut usize) -> Paragraph<'a> {
    let mut text = vec![
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Welcome")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("to")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "Typing Game",
            Style::default().fg(Color::LightBlue),
        )]),
        Spans::from(vec![Span::raw("")]),
    ];
    if start && *countdown > 0 {
        thread::sleep(Duration::from_secs(1));
        let mut s = String::from("Starting race in ");
        s.push_str(countdown.to_string().as_str());
        let span = Spans::from(vec![Span::raw(s.as_str().to_owned())]);
        text.push(span);
        *countdown = *countdown - 1;
    }
    let home = Paragraph::new(text)
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Home")
            .border_type(BorderType::Plain),
    );
    home
}
