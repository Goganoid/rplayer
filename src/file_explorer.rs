use std::cell::Cell;
use crate::{graphics, relative_size};
use std::io::Stdout;
use crate::file_manager::FileManager;
use crossterm::style::{style};
use crossterm::style::Colorize;

use lru_cache::LruCache;
use std::ops::Deref;
use std::path::PathBuf;

pub struct FileExplorer{
    files: usize,
    viewport_size: usize,
    highlighted_index:Cell<usize>,
    start_index: Cell<usize>,
    cache: LruCache<PathBuf,String>,
}
impl FileExplorer{
    pub fn new(files:usize,mut viewport_size:usize) -> FileExplorer{
        if files<viewport_size{
            viewport_size = files;
        }
        let file_explorer = FileExplorer{files,viewport_size,start_index:Cell::new(0),highlighted_index:Cell::new(0),cache:LruCache::new(100)};
        file_explorer.set_viewport(0);
        file_explorer
    }
    fn max_start_index_value(&self) -> usize{
        self.files - self.viewport_size
    }
    pub fn set_viewport(&self,mut start_index:usize){
        let max_value = self.max_start_index_value();

        if(start_index)> max_value{
            start_index = max_value;
        }
        self.start_index.set(start_index);
    }
    pub fn move_viewport_down(&self){
        if self.start_index.get()<self.max_start_index_value(){
            self.set_viewport(self.start_index.get()+1);
        }
    }
    pub fn move_viewport_up(&self){
        if self.start_index.get()>0{
            self.set_viewport(self.start_index.get()-1);
        }
    }
    pub fn move_highlight_by(&self,n:i32){
        let new_index= self.highlighted_index.get() as i32 + n;
        if new_index >=0 && new_index <self.files as i32{
            self.highlighted_index.set(new_index as usize);
        }

    }
    pub fn set_highlight(&self,position:usize){
        if position <self.files as usize {
            self.highlighted_index.set(position);
        }
    }
    pub fn move_down(&self){
        let index = self.highlighted_index.get();
        if index-self.start_index.get() == self.viewport_size-1{
            self.move_viewport_down();
        }
        self.move_highlight_by(1);
    }
    pub fn move_up(&self){
        if self.highlighted_index.get()-self.start_index.get() == 0{
            self.move_viewport_up();
        }
        self.move_highlight_by(-1);

    }



    pub fn draw(&mut self, stdout:&mut Stdout, file_manager:&mut FileManager, x:u16, mut y:u16, width:u16){
        let start_index = self.start_index.get();
        let end_index = start_index + self.viewport_size;
        let padding = relative_size(1.0/4.0,width) as usize;
        let mut header = style(format!("{} {} {} {}",
                                       graphics::set_text_width(String::from("Filename"),padding),
                                       graphics::set_text_width(String::from("Track"),padding),
                                       graphics::set_text_width(String::from("Artist"),padding),
                                       graphics::set_text_width(String::from("Duration"),padding)
        ));
        header = header.on_dark_blue();
        graphics::draw_text(stdout,header,x,y as u16).unwrap();
        y+=1;
        for (index,path) in file_manager.get_paths(start_index,end_index).iter().enumerate(){

            let file_info = match self.cache.get_mut(path){
                Some(desc) =>desc.deref().clone(),
                None =>{
                    let meta = file_manager.get_metadata(&path);
                    let time = graphics::set_text_width(format!("{}",graphics::duration_to_mmss(meta.duration)),padding);
                    let mut title = String::from(" ");
                    let mut artist = String::from(" ");
                    if meta.optional_info.len()!=0{
                        if let Some(maybe_title) = meta.optional_info[0].title.as_ref(){
                            title = maybe_title.clone().trim_matches(char::from(0)).to_string();
                        }
                    }
                    if let Some(tag) = meta.tag.as_ref(){
                        artist = tag.artist.clone().trim_matches(char::from(0)).to_string();
                    }
                    title = graphics::set_text_width(title, padding);
                    artist = graphics::set_text_width(artist, padding);

                    let desc = format!("{} {} {} {}",
                                                        graphics::set_text_width(String::from(path.file_name().unwrap().to_str().unwrap_or("ERROR READING!")),padding-5),
                                                        title,
                                                        artist,
                                                        time
                    );
                    self.cache.insert(path.clone(),desc.clone());
                    desc
                }
            };
            let display_index = graphics::set_text_width(format!("{}.",index+self.start_index.get()),5);
            let mut description = style(format!("{}{}",display_index,file_info));
            if index+start_index == self.highlighted_index.get(){
                description = description.on_blue();
            }
            graphics::draw_text(stdout,description,x,y+index as u16).unwrap();
        }

    }
}