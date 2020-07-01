#![recursion_limit="1024"]

use std::io::{Write,Stdout,stdout};
use std::time::{Instant,Duration};
use std::thread::sleep;
use std::thread::spawn;
use std::sync::mpsc;
use futures::{future::FutureExt, select, StreamExt};


use crossterm::{Result, execute, terminal, style::Colorize, terminal::{EnterAlternateScreen, LeaveAlternateScreen, enable_raw_mode, disable_raw_mode, Clear, ClearType}, event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers}, cursor::{Hide, Show}, QueueableCommand};
use crate::audio_controller::AudioPlayer;
use std::path::{PathBuf};
use crossterm::style::style;
use crate::graphics::{Square, draw_text, Drawable};
use crate::file_explorer::FileExplorer;


pub mod audio_controller;
pub mod file_manager;
pub mod graphics;
pub mod file_explorer;
struct FramerateClock{
    framerate: u64,
    before: Instant,
}
impl FramerateClock{
    fn new(fps:u64) -> FramerateClock{
        FramerateClock{
            framerate:1000/fps,
            before:Instant::now()}
    }
    fn sleep_if_needed(&mut self){
        let now = Instant::now();
        let dt = now.duration_since(self.before).as_millis();
        self.before = now;
        if dt <= self.framerate as u128 {
            sleep(Duration::from_millis(self.framerate - dt as u64));
        }
    }
}

pub enum AppAction{
    Exit,
    ChangeTrack(i32),
    NextTrack,
    PrevTrack,
    MoveTimestampBySecs(i32),
    IncreaseVolume,
    DecreaseVolume,
    Start,
    Pause,
    Shuffle,
}

fn relative_size(val:f32, size:u16) -> u16{
    (val*size as f32) as u16
}
pub struct App{
    stdout: Stdout,
    path: PathBuf,
    size:(u16,u16),
}
impl App {
    pub fn new(path:PathBuf) -> App {
        App {
            stdout: stdout(),
            path,
            size: terminal::size().unwrap()
        }
    }
    pub async fn process_key_events(sender:mpsc::Sender<AppAction>){
        let mut reader = EventStream::new();
        loop {
            let mut event = reader.next().fuse();
            select! {
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {

                       if event == Event::Key(KeyCode::Up.into()) {
                            sender.send(AppAction::IncreaseVolume).unwrap();
                        }
                        if event == Event::Key(KeyCode::Down.into()) {
                            sender.send(AppAction::DecreaseVolume).unwrap();
                        }

                        if event == Event::Key(KeyCode::Char('s').into()) {
                            sender.send(AppAction::Shuffle).unwrap();
                        }
                        if event == Event::Key(KeyEvent{
                                            code:KeyCode::Right,
                                            modifiers:KeyModifiers::CONTROL}) || event == Event::Key(KeyCode::Char('d').into())
                        {
                            sender.send(AppAction::MoveTimestampBySecs(5)).unwrap();
                        }
                        if event == Event::Key(KeyEvent{
                                            code:KeyCode::Left,
                                            modifiers:KeyModifiers::CONTROL}) || event == Event::Key(KeyCode::Char('a').into())
                        {
                            sender.send(AppAction::MoveTimestampBySecs(-5)).unwrap();
                        }
                        if event == Event::Key(KeyEvent{
                                            code:KeyCode::Right,
                                            modifiers:KeyModifiers::ALT})
                        {
                            sender.send(AppAction::ChangeTrack(5)).unwrap();
                        }
                        if event == Event::Key(KeyEvent{
                                            code:KeyCode::Left,
                                            modifiers:KeyModifiers::ALT})
                        {
                            sender.send(AppAction::ChangeTrack(-5)).unwrap();
                        }
                        if event == Event::Key(KeyCode::Right.into()) {
                            sender.send(AppAction::ChangeTrack(1)).unwrap();
                        }
                        if event == Event::Key(KeyCode::Left.into()) {
                            sender.send(AppAction::ChangeTrack(-1)).unwrap();
                        }
                        if event == Event::Key(KeyCode::Char(' ').into()) {
                            sender.send(AppAction::Start).unwrap();
                        }
                        if event == Event::Key(KeyCode::Esc.into()) {
                            sender.send(AppAction::Exit).unwrap();
                            break;
                        }

                    }
                    Some(Err(e)) => println!("Error: {:?}\r", e),
                    None => break,
                }
            }
        }
        }
    }
    fn match_key_actions(action:AppAction,audio_player:&mut AudioPlayer,file_explorer:&mut FileExplorer){
        match action {
            AppAction::Start => {
                if !audio_player.is_running() {
                    audio_player.run().unwrap();
                    std::thread::sleep(Duration::from_millis(20));
                } else {
                    if !audio_player.is_playing() {
                        audio_player.play();
                    } else {
                        audio_player.pause();
                    }
                }
            }
            AppAction::Pause => audio_player.pause(),
            AppAction::ChangeTrack(n) =>{
                if n>=0{
                    for _ in 0..n{
                        file_explorer.move_down();
                        audio_player.set_next_track_in_dir();
                    }
                }
                else{
                    for _ in 0..(-n){
                        file_explorer.move_up();
                        audio_player.set_prev_track_in_dir();
                    }
                }
            }
            AppAction::NextTrack => {
                file_explorer.move_down();
                audio_player.set_next_track_in_dir();
            },
            AppAction::PrevTrack => {
                file_explorer.move_up();
                audio_player.set_prev_track_in_dir();
            },
            AppAction::MoveTimestampBySecs(delta) => {
                if delta < 0 {
                    audio_player.move_timestamp_back(Duration::from_secs(-delta as u64));
                } else {
                    audio_player.move_timestamp_forward(Duration::from_secs(delta as u64));
                }
            },
            AppAction::Shuffle => {
                audio_player.file_manager.toggle_shuffle();
                if !audio_player.file_manager.is_shuffled() {
                    let index = audio_player.file_manager.current_index();
                    file_explorer.set_viewport(index);
                    file_explorer.set_highlight(index);
                }
            }
            AppAction::IncreaseVolume => audio_player.increase_volume_by(0.15),
            AppAction::DecreaseVolume => audio_player.decrease_volume_by(0.15),
            _=>(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        // min screen size
        if self.size.0>80 && self.size.1>5 {
            execute!(
                self.stdout,
                EnterAlternateScreen,
                Hide,
            ).unwrap();
            enable_raw_mode().unwrap();

            let square = Square::new(relative_size(0.8, self.size.0), relative_size(0.7, self.size.1));
            let mut volume_display = graphics::VolumeDisplay::new(relative_size(0.05, self.size.0), 3.0);
            let mut time_slider = graphics::TimeSlider::new(relative_size(0.5, self.size.0));
            let (tx, rx) = mpsc::channel();
            let key_thread = spawn(|| {
                async_std::task::block_on(App::process_key_events(tx));
            });
            if let Ok( mut audio_player) =  AudioPlayer::new(self.path.as_path()){
                let mut file_explorer = FileExplorer::new(audio_player.file_manager.file_paths.len(), square.height as usize - 2);
                let mut clock = FramerateClock::new(15);
                loop {
                    match audio_player.get_track_meta() {
                        Some(meta) => {
                            time_slider.set_duration(Some(meta.duration));
                            time_slider.set_timestamp(audio_player.get_timestamp());
                        }
                        None => {
                            time_slider.set_duration(None);
                            time_slider.set_timestamp(Duration::from_secs(0));
                        }
                    }

                    self.stdout.queue(Clear(ClearType::All))?;
                    // draw interface
                    square.draw(&mut self.stdout, 0, 0)?;

                    volume_display.set_volume(audio_player.get_volume());
                    volume_display.draw(&mut self.stdout, relative_size(0.01, self.size.0), relative_size(0.85, self.size.1))?;

                    time_slider.draw(&mut self.stdout, relative_size(0.2, self.size.0), relative_size(0.85, self.size.1))?;

                    let mut state = style(graphics::graphic_symbols::PAUSE).red();
                    if audio_player.is_playing() {
                        state = style(graphics::graphic_symbols::PLAY).green();
                    }
                    draw_text(&mut self.stdout, state, relative_size(0.2, self.size.0) + time_slider.slider.length() + 18, relative_size(0.85, self.size.1))?;

                    let mut is_shuffled_text = style("SHUFFlE").dark_grey();
                    if audio_player.file_manager.is_shuffled(){
                        is_shuffled_text = is_shuffled_text.white();
                    }
                    draw_text(&mut self.stdout, is_shuffled_text, relative_size(0.8, self.size.0)+2, relative_size(0.03, self.size.1))?;

                    file_explorer.draw(&mut self.stdout, &mut audio_player.file_manager, 1, 1, relative_size(0.8, self.size.0 - 6));
                    self.stdout.flush()?;

                    // if sample is None then the current track has finished
                    if let None = audio_player.get_current_sample() {
                        if audio_player.current_track_is_active() {
                            if audio_player.file_manager.tracks_left() != 0 {
                                audio_player.set_next_track_in_dir();
                                file_explorer.move_down();
                            } else { audio_player.pause() }
                        }
                    }
                    // get input
                    if let Ok(action) = rx.try_recv() {
                        match action {
                            AppAction::Exit => break,
                            _ => App::match_key_actions(action,&mut audio_player,&mut file_explorer)
                        }
                    }
                    clock.sleep_if_needed();
                }
                audio_player.stop();
            }
            else{
                println!("\rPress Esc");
            }
            key_thread.join().unwrap();
            disable_raw_mode().unwrap();
            execute!(
                self.stdout,
                LeaveAlternateScreen,
                Show
            ).unwrap();
        }
        else{
            println!("Screen size is too small!");
        }
        Ok(())
    }
}