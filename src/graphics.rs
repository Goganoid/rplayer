extern crate unicode_width;
use unicode_width::UnicodeWidthStr;

use crossterm::{QueueableCommand, cursor,Result,style};
use std::io::{Stdout};
use std::fmt::Display;
use std::cell::Cell;
use std::time::Duration;

pub mod graphic_symbols {
    pub const DOUBLE_TOP_LEFT_CORNER: &'static str = "╔";
    pub const DOUBLE_TOP_RIGHT_CORNER: &'static str = "╗";
    pub const DOUBLE_BOTTOM_LEFT_CORNER: &'static str = "╚";
    pub const DOUBLE_BOTTOM_RIGHT_CORNER: &'static str = "╝";
    pub const DOUBLE_VERTICAL_LINE: &'static str = "║";
    pub const DOUBLE_HORIZONTAL_LINE: &'static str = "═";
    pub const CELL: &'static str = "■";
    pub const DOUBLE_LINE_VERTICAL_AND_LEFT:&'static str = "╣";
    pub const SINGLE_HORIZONTAL_LINE:&'static str = "─";
    pub const PAUSE:&'static str = "PAUSE";
    pub const PLAY:&'static str = "PLAY";
    pub const SHUFFLE:&'static str = "🔀️🔀️🔀️🔀️";

}

pub fn set_text_width(mut text:String,width:usize) -> String{
    let text_vec =  text.chars().collect::<Vec<_>>();
    let text_size = UnicodeWidthStr::width(text.as_str());
    let mut new_text = String::new();
    if text_size<=width{
        text = format!("{}{}",text," ".repeat(width-text_vec.len()));
    }
    else{
        for char in text_vec{
            if UnicodeWidthStr::width(new_text.as_str())<width-3{
                new_text.push(char);
            }
            else{ break;}
        }
        new_text.push_str(".".repeat(width-UnicodeWidthStr::width(new_text.as_str())).as_str());
        text = new_text;
    }
    text
}
pub fn duration_to_mmss(duration:Duration) -> String{
    let minutes_string = (duration.as_secs()/60).to_string();
    let seconds = duration.as_secs_f64()%60.0;
    let seconds_string = if seconds<10.0{
        format!("0{:.1}",seconds)
    }
    else{
        format!("{:.1}",seconds)
    };
    format!("{}:{}",minutes_string,seconds_string)
}

pub trait Drawable{
    fn draw(&self,stdout:&mut Stdout,x:u16,y:u16) -> Result<()>;
}

pub struct Square{
    pub width:u16,
    pub height:u16,
}
impl Square{
    pub fn new(width:u16, height:u16) -> Square{
        Square{width,height}
    }
}
impl Drawable for Square{
    fn draw(&self,stdout:&mut Stdout,x:u16,y:u16) -> Result<()>{
        stdout
            .queue(cursor::MoveTo(x,y))?
            .queue(style::Print(graphic_symbols::DOUBLE_TOP_LEFT_CORNER))?
            .queue(style::Print(graphic_symbols::DOUBLE_HORIZONTAL_LINE.repeat(self.width as usize)))?
            .queue(cursor::MoveTo(x+self.width,y))?
            .queue(style::Print(graphic_symbols::DOUBLE_TOP_RIGHT_CORNER))?
            .queue(cursor::MoveTo(x,y+self.height))?
            .queue(style::Print(graphic_symbols::DOUBLE_BOTTOM_LEFT_CORNER))?
            .queue(style::Print(graphic_symbols::DOUBLE_HORIZONTAL_LINE.repeat(self.width as usize)))?
            .queue(cursor::MoveTo(x+self.width,x+self.height))?
            .queue(style::Print(graphic_symbols::DOUBLE_BOTTOM_RIGHT_CORNER))?;
        // draw vertical lines
        for row in 1..self.height{
            stdout
                .queue(cursor::MoveTo(x,y+row))?
                .queue(style::Print(graphic_symbols::DOUBLE_VERTICAL_LINE))?
                .queue(cursor::MoveTo(x+self.width,y+row))?
                .queue(style::Print(graphic_symbols::DOUBLE_VERTICAL_LINE))?;
        }

        Ok(())
    }
}


pub struct Slider{
    length:u16,
    pos:Cell<f32>
}
impl Slider{
    pub fn new(length:u16) -> Slider{
        Slider{length,pos:Cell::new(0.0)}
    }
    pub fn length(&self) -> u16{
        self.length
    }
    pub fn set_pos(&self,mut pos:f32){
        if pos>1.0 { pos=1.0}
        self.pos.set(pos);
    }
}
impl Drawable for Slider{
    fn draw(&self,stdout:&mut Stdout,x:u16,y:u16) -> Result<()> {
        draw_text(stdout, graphic_symbols::SINGLE_HORIZONTAL_LINE.repeat(self.length as usize).as_str(), x, y).unwrap();
        let cell_x = (self.pos.get() * self.length as f32) as u16;
        draw_text(stdout, graphic_symbols::CELL, x + cell_x, y).unwrap();
        Ok(())
    }
}
pub struct VolumeDisplay{
    slider:Slider,
    max_volume_value:f32,
    volume:f32,
}
impl VolumeDisplay{
    pub fn new(slider_length:u16, max_volume_value:f32) -> VolumeDisplay{
        VolumeDisplay{slider:Slider::new(slider_length),max_volume_value,volume:0.0}
    }
    pub fn set_volume(&mut self,volume:f32){
        self.volume = volume;
    }
}
impl Drawable for VolumeDisplay{
    fn draw(&self,stdout:&mut Stdout,x:u16,y:u16) -> Result<()>{
        let volume_text = format!("Volume:{:.0}% ",self.volume*100.0);
        draw_text(stdout,volume_text.as_str(),x,y).unwrap();
        let pos = self.volume/self.max_volume_value;
        self.slider.set_pos(pos);
        self.slider.draw(stdout,x+12,y).unwrap();
        Ok(())
    }
}
pub struct TimeSlider{
    pub slider:Slider,
    duration: Option<Duration>,
    timestamp:Duration
}
impl TimeSlider{
    pub fn new(slider_length:u16) -> TimeSlider{
        TimeSlider{slider:Slider::new(slider_length),duration:None,timestamp:Duration::from_secs(0)}
    }
    pub fn set_duration(&mut self,duration:Option<Duration>){
        self.duration = duration;
    }
    pub fn set_timestamp(&mut self,timestamp:Duration){
        self.timestamp = timestamp
    }
}
impl Drawable for TimeSlider{
    fn draw(&self,stdout:&mut Stdout,mut x:u16,y:u16) -> Result<()>{
        match self.duration.as_ref(){
            Some(duration) => {
                self.slider.set_pos(self.timestamp.as_secs_f32()/duration.as_secs_f32())
            },
            None => self.slider.set_pos(0.0)
        }
        let timestamp_string = duration_to_mmss(self.timestamp);
        draw_text(stdout,&timestamp_string,x,y).unwrap();
        let padding = 2;
        x+= timestamp_string.len() as u16 + padding;
        self.slider.draw(stdout, x, y).unwrap();
        x+= self.slider.length+padding;
        draw_text(stdout,duration_to_mmss(self.duration.unwrap_or(Duration::from_secs(0))),x,y).unwrap();
        Ok(())
    }
}


pub fn draw_text<T:Display+Clone>(stdout:&mut Stdout,text:T,x:u16,y:u16) -> Result<()>{
    stdout
        .queue(cursor::MoveTo(x,y))?
        .queue(style::Print(text))?;
    Ok(())
}

