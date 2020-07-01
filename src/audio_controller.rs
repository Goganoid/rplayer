extern crate cpal;
extern crate minimp3;
use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};
use minimp3::{Decoder, Frame};
use std::fs::File;

use std::vec::IntoIter;
    use cpal::{Sample};
use self::cpal::{StreamId, EventLoop, Device, Format};
use std::sync::{Mutex};
use std::sync::Arc;
use std::thread::spawn;
use std::time::{Duration};
use std::ops::{Deref, DerefMut};
use crate::file_manager::FileManager;
use std::path::{Path, PathBuf};
use std::cell::{RefCell, Ref};
use mp3_metadata::{MP3Metadata};
use std::borrow::BorrowMut;


pub struct TrackData {
    path: PathBuf,
    decoder:Decoder<File>,
    iterator:Option<IntoIter<i16>>,
    prev_frames:Vec<Frame>,
    frame_index:usize,
    frames_passed:usize,
    max_time_stamp:Duration,
    sample_rate:i32,
    channels:usize,
    frames:usize,
    duration: Duration,
    current_sample:Option<i16>,
    timestamp:Duration,
    sample_duration:Duration,
    is_active: bool,

}
impl TrackData {
    pub fn new(path:PathBuf) -> TrackData {
        let file = File::open(&path).unwrap();
        let meta = mp3_metadata::read_from_file(&path).unwrap();
        let mut decoder = Decoder::new(file.try_clone().unwrap());

        // get sample rate, number of channels and first frame data
        let first_frame = decoder.next_frame().unwrap();
        let sample_rate = first_frame.sample_rate;
        let channels = first_frame.channels;
        let iterator = Some(first_frame.data.clone().into_iter());

        let sample_duration = Duration::from_nanos(100_000_0000 / (sample_rate*channels as i32) as u64);

        let duration = meta.duration;
        let frames = meta.frames.len();

        let mut packet_decoder = TrackData {
            decoder,
            iterator,
            prev_frames:Vec::new(),
            frame_index:0,
            frames_passed:0,
            max_time_stamp:Duration::from_secs(0),
            timestamp:Duration::from_secs(0),
            sample_duration,
            sample_rate,
            channels,
            duration,
            frames,
            current_sample:None,
            is_active:false,
            path
        };


        TrackData::set_timestamp(&mut packet_decoder, Duration::from_secs(0));
        packet_decoder
    }
    pub fn get_sample(&mut self) -> Option<i16> {
        let mut result = None;
        // get data from next frames
        if self.timestamp==self.max_time_stamp {
            if let Some(iterator) = &mut self.iterator {
                match iterator.next() {
                    Some(sample) => {
                        result = Some(sample);
                        self.timestamp += self.sample_duration;
                        self.is_active = true;
                    },
                    // no samples left in frame => get next frame
                    None => {
                        if let Ok(next_frame) = self.decoder.next_frame() {
                            self.frames_passed += 1;
                            self.prev_frames.push(next_frame.clone());
                            self.iterator = Some(next_frame.data.into_iter());
                            result = self.get_sample();
                        }
                    }
                }
                self.max_time_stamp = self.timestamp;
            }
        }
        // get data from previous frames
        else{
            if let Some(iterator) = &mut self.iterator {
                match iterator.next() {
                    Some(sample) => {
                        result = Some(sample);
                        self.timestamp += self.sample_duration;
                        self.is_active = true;
                    },
                    None => {
                        self.iterator = Some(self.prev_frames[self.frame_index].data.clone().into_iter());
                        self.frame_index+=1;
                        result = self.get_sample();
                    }
                }
            }
        }
        self.current_sample = result.clone();
        result

    }
    pub fn set_timestamp(&mut self, timestamp:Duration){
        self.timestamp = timestamp;

        let frame_at_timestamp = ((timestamp.as_secs_f64()/self.duration.as_secs_f64()) as f64 * self.frames as f64) as usize;

        if self.timestamp>=self.max_time_stamp{
            self.max_time_stamp = self.timestamp;
            for _ in 0..frame_at_timestamp -self.frames_passed{
                if let Ok(frame) = self.decoder.next_frame(){
                    self.frames_passed+=1;
                    self.prev_frames.push(frame);
                }
            }
        }
        else{
            self.frame_index = frame_at_timestamp;
            self.iterator = Some(self.prev_frames[self.frame_index].data.clone().into_iter());
        }
    }
}

pub struct AudioPlayer {
    format: Format,
    device: Device,
    volume: Arc<Mutex<f32>>,
    current_track: Option<Arc<Mutex<TrackData>>>,
    current_track_meta:Option<RefCell<MP3Metadata>>,
    event_loop:Arc<EventLoop>,
    stream_id: Arc<StreamId>,
    pub file_manager: FileManager,
    is_running:RefCell<bool>,
    is_playing:RefCell<bool>,
}
impl AudioPlayer {
    pub fn new(dir:&Path) -> Result<AudioPlayer,()> {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("failed to find a default output device");
        if let Ok(file_manager) = FileManager::new(dir){
            let current_track = TrackData::new(file_manager.get_current());
            let mut format = device.default_output_format().unwrap();
            format.channels = current_track.channels as u16;
            let event_loop = Arc::new(host.event_loop());
            let stream_id = Arc::new(event_loop.build_output_stream(&device,&format).unwrap());

            Ok(AudioPlayer {
                device,
                format,
                event_loop,
                stream_id,
                volume:Arc::new(Mutex::new(1.0)),
                current_track:Some(Arc::new(Mutex::new(current_track))),
                current_track_meta:Some(RefCell::new(mp3_metadata::read_from_file(file_manager.get_current()).unwrap())),
                file_manager,
                is_running:RefCell::new(false),
                is_playing:RefCell::new(false),
            })
        }
        else{
            Err(())
        }

    }
    pub fn run(&mut self) -> Result<(), anyhow::Error> {
        *self.is_running.borrow_mut() = true;
        self.play();

        let event_loop_clone = self.event_loop.clone();
        let volume_clone = self.volume.clone();
        let current_track_clone = self.current_track.as_ref().unwrap().clone();

        spawn(move || {
            event_loop_clone.run(move |id, result| {
                let data = match result {
                    Ok(data) => data,
                    Err(err) => {
                        eprintln!("an error occurred on stream {:?}: {}", id, err);
                        return;
                    }
                };
                match data {
                    cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::F32(mut buffer) } => { AudioPlayer::write_data(&mut buffer,current_track_clone.lock().unwrap().deref_mut(), *volume_clone.lock().unwrap()) },
                    cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::U16(mut buffer) } => { AudioPlayer::write_data(&mut buffer,current_track_clone.lock().unwrap().deref_mut(), *volume_clone.lock().unwrap()); },
                    cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::I16(mut buffer) } => { AudioPlayer::write_data(&mut buffer,current_track_clone.lock().unwrap().deref_mut(), *volume_clone.lock().unwrap()); },
                    _ => ()
                }
            });
        });
        Ok(())
    }
    pub fn is_running(&self) -> bool{
        *self.is_running.borrow()
    }
    pub fn set_volume(&mut self,volume:f32){ *self.volume.lock().unwrap()=volume; }
    pub fn increase_volume_by(&mut self,volume:f32){
        let new_volume =  *self.volume.lock().unwrap()+volume;
        if new_volume<3.0 { *self.volume.lock().unwrap() = new_volume;}
    }
    pub fn decrease_volume_by(&mut self,volume:f32){
        let new_volume =  *self.volume.lock().unwrap()-volume;
        if new_volume>0.0 { *self.volume.lock().unwrap() = new_volume;}
    }
    pub fn get_volume(&self) -> f32{
        *self.volume.lock().unwrap()
    }
    pub fn current_track_is_active(&self) -> bool{
        self.current_track.as_ref().unwrap().lock().unwrap().is_active
    }
    pub fn set_timestamp(&self,timestamp:Duration){
        self.pause();
        std::thread::sleep(Duration::from_millis(5));
        self.current_track.as_ref().unwrap().lock().unwrap().set_timestamp(timestamp);
        self.play();
    }
    pub fn get_current_sample(&self) -> Option<i16>{
        self.current_track.as_ref().unwrap().lock().unwrap().current_sample.clone()
    }
    pub fn move_timestamp_forward(&self,timestamp_delta:Duration){
        let current_track = self.current_track.as_ref().unwrap().lock().unwrap();
        let mut timestamp = timestamp_delta+current_track.timestamp;
        if timestamp>current_track.duration {
            timestamp = current_track.duration
        }
        std::mem::drop(current_track);

        self.set_timestamp(timestamp);
    }
    pub fn move_timestamp_back(&self,timestamp_delta:Duration){
        let current_track = self.current_track.as_ref().unwrap().lock().unwrap();
        let mut timestamp = Duration::from_secs(0);
        if timestamp_delta<current_track.timestamp
        {
            timestamp = current_track.timestamp - timestamp_delta;
        }
        std::mem::drop(current_track);
        self.set_timestamp(timestamp);
    }
    pub fn get_timestamp(&self) -> Duration{
        match self.current_track.as_ref(){
            Some(track) =>{
                track.lock().unwrap().timestamp
            },
            None => Duration::from_secs(0)
        }
    }
    pub fn get_track_meta(&self) -> Option<Ref<'_, MP3Metadata>> {
        match self.current_track_meta.as_ref(){
            Some(meta) =>Some(meta.borrow()),
            None => None,
        }
    }
    pub fn change_track(&mut self,track: TrackData){
        let path= track.path.clone();
        *self.current_track.as_ref().unwrap().lock().unwrap() = track;
        self.current_track_meta.as_ref().unwrap().replace(mp3_metadata::read_from_file(path).unwrap());
        self.rebuild_stream();
    }
    pub fn set_next_track_in_dir(&mut self){
        if let Some(path) = self.file_manager.next(){
            let track = TrackData::new(path);
            self.change_track(track);
        }
    }
    pub fn set_prev_track_in_dir(&mut self){
        if let Some(path) = self.file_manager.prev(){
            let track = TrackData::new(path);
            self.change_track(track);
        }
    }
    pub fn is_playing(&self) -> bool{
        *self.is_playing.borrow()
    }
    pub fn pause(&self){
        *self.is_playing.borrow_mut() = false;
        self.event_loop.pause_stream(self.stream_id.deref().clone()).unwrap();
    }
    pub fn play(&self){
        *self.is_playing.borrow_mut() = true;
        self.event_loop.play_stream(self.stream_id.deref().clone()).unwrap();
    }
    fn rebuild_stream(&mut self){
        self.event_loop.destroy_stream(self.stream_id.deref().clone());
        self.format.borrow_mut().channels = self.current_track.as_ref().unwrap().lock().unwrap().channels as u16;
        self.stream_id = Arc::new(self.event_loop.build_output_stream(&self.device,&self.format).unwrap());
        self.play();
    }
    pub fn stop(&self){
        *self.is_running.borrow_mut() = false;
        self.event_loop.destroy_stream(self.stream_id.deref().clone());
    }

    fn write_data<T>(output: &mut cpal::OutputBuffer<T>, value_iterator: &mut TrackData, volume:f32)
        where
            T: Sample,
    {
        for frame in output.chunks_mut(value_iterator.channels) {

            for sample in frame.iter_mut() {
                let mut value = match value_iterator.get_sample() {
                    Some(data) => data as f32 / value_iterator.sample_rate as f32,
                    None => {
                        0.0;
                        break
                    }
                };
                value*=volume;
                *sample = T::from(&value);
            }
        }
    }
}
