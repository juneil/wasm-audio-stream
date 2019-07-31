#[macro_use]
extern crate lazy_static;

use std::sync::Mutex;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{ErrorEvent, MessageEvent, WebSocket, BinaryType, AudioContext, AudioBuffer};
use js_sys::{Int16Array, Reflect, Number};

static MIN_SPLIT_SIZE: f32 = 0.02;
static MAX_SAMPLE_VALUE: usize = 32768;

macro_rules! console_log {
    ($($t:tt)*) => (log(&format!("[wasm] {}", &format_args!($($t)*).to_string()).to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub struct AudioConfig {
    channels: u8,
    rate: usize,
    bytes: usize,
    uri: String
}

#[wasm_bindgen]
impl AudioConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(uri: String, channels: u8, rate: usize, bytes: usize) -> AudioConfig {
        AudioConfig {
            uri: uri,
            channels: channels,
            rate: rate,
            bytes: bytes
        }
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct AudioState {
    ws: Option<WebSocket>,
    ctx: AudioContext,
    channels: u8,
    rate: usize,
    bytes: usize,
    next_time: usize,
    queue: Vec<Int16Array>
}

impl AudioState {
    pub fn set_state(&mut self, config: AudioConfig) {
        self.ws = WebSocket::new_with_str(&config.uri, "binary").ok();
        self.channels = config.channels;
        self.rate = config.rate;
        self.bytes = config.bytes;
        self.next_time = 0;
        self.queue = vec!();
    }

    pub fn set_queue(&mut self, queue: Vec<Int16Array>) {
        self.queue = queue;
    }

    pub fn set_next_time(&mut self, time: usize) {
        self.next_time = time;
    }

    pub fn close(&self) {
        match self.ws {
            Some(_ws) => match _ws.close() {
                Err(e) => console_log!("{:?}", e),
                _ => ()
            },
            _ => console_log!("Impossible the close the websocket: no instance")
        }
    }
}

lazy_static! {
    static ref STATE: Mutex<AudioState> = Mutex::new(AudioState {
        ws: None,
        ctx: AudioContext::new().ok().unwrap(),
        channels: 0,
        rate: 0,
        bytes: 0,
        next_time: 0,
        queue: vec!()
    });
}

#[wasm_bindgen]
pub fn start(config: Option<AudioConfig>) {
    console_log!("Audio stream is starting...");
    let state = match config {
        Some(conf) => STATE.set_state(conf),
        None => panic!("You should provide a configuration")
    };
    init_ws();
}

#[wasm_bindgen]
pub fn stop() {
    console_log!("Socket closed");
    STATE.close();
}

/**
 * WebSocket initialisation
 * Set binary type to arraybuffer
 * Bind the callbacks
**/
fn init_ws() {
    console_log!("Websocket initialisation");
    match STATE.ws {
        Some(ws) => {
            ws.set_binary_type(BinaryType::Arraybuffer);
            console_log!("Binary type: {:?}", ws.binary_type());
            let on_message_cb = on_message();
            let on_open_cb = on_open();
            let on_error_cb = on_error();
            ws.set_onmessage(Some(on_message_cb.as_ref().unchecked_ref()));
            ws.set_onopen(Some(on_open_cb.as_ref().unchecked_ref()));
            ws.set_onerror(Some(on_error_cb.as_ref().unchecked_ref()));
            on_message_cb.forget();
            on_open_cb.forget();
            on_error_cb.forget();
        },
        _ => console_log!("Cannot init WS: no instance")
    }
}

/**
 * WebSocket MESSAGE callback
**/
fn on_message() -> Closure<dyn FnMut(MessageEvent)> {
    Closure::wrap(Box::new(move |e: MessageEvent| {
        let response = e.data();
        let data = Int16Array::new_with_byte_offset(&response, 0);
        STATE.queue.push(data);
        let packet = shift_packet();

        console_log!("Message event, received data: {:?}", response);
    }) as Box<dyn FnMut(MessageEvent)>)
}

/**
 * WebSocket OPEN callback
**/
fn on_open() -> Closure<dyn FnMut(JsValue)> {
    Closure::wrap(Box::new(move |_| {
        console_log!("Socket opened");
    }) as Box<dyn FnMut(JsValue)>)
}

/**
 * WebSocket ERROR callback
**/
fn on_error() -> Closure<dyn FnMut(ErrorEvent)> {
    Closure::wrap(Box::new(move |e: ErrorEvent| {
        console_log!("Error event: {:?}", e);
    }) as Box<dyn FnMut(ErrorEvent)>)
}

fn join_packets(packets: Vec<Int16Array>) -> Int16Array {
    if packets.len() <= 1 {
        return packets.get(0).unwrap().clone();
    }
    let length = packets
        .iter()
        .map(|packet| packet.length())
        .fold(0, |acc, cur| acc + cur);

    let joined = Int16Array::new_with_length(length);
    let mut offset = 0;

    packets
        .iter()
        .for_each(|packet| {
            joined.set(packet, offset);
            offset += packet.length();
        });

    joined
}

fn split_packet(data: Int16Array) -> Vec<Int16Array> {
    let mut min_value = std::u32::MAX;
    let mut optimal_value = data.length();
    let samples = ((data.length() / STATE.channels as u32) as f32).floor() as u32;
    let min_split_samples = (STATE.rate as f32 * MIN_SPLIT_SIZE).floor() as u32;
    let start = [
        STATE.channels as u32 * min_split_samples,
        STATE.channels as u32 * (samples - min_split_samples)
    ]
    .iter()
    .max()
    .unwrap()
    .clone();

    let mut offset = start;
    while offset < data.length() {
        let mut total = 0;
        for channel in 0..STATE.channels {
            let value = Reflect::get(&data, &Number::from(offset + channel as u32)).ok().unwrap();
            total = total + value
                .as_f64()
                .map(|x| x as i32)
                .unwrap()
                .abs();
        }
        if (total as u32) <= min_value {
            optimal_value = offset + STATE.channels as u32;
            min_value = total as u32;
        }
        offset = offset + STATE.channels as u32;
    };

    if optimal_value == data.length() {
        return vec!(data);
    }

    let buf1 = data.buffer().slice_with_end(0, optimal_value * (STATE.bytes as u32));
    let buf2 = data.buffer().slice(optimal_value * (STATE.bytes as u32));
    vec!(
        Int16Array::new_with_byte_offset(&buf1, 0),
        Int16Array::new_with_byte_offset(&buf2, 0)
    )
}

fn shift_packet() -> Option<Int16Array> {
    let data = join_packets(STATE.queue.clone());
    let new_queue = split_packet(data);
    STATE.set_queue(new_queue);
    STATE.queue.first().map(|x| x.clone())
}

fn to_audio_buffer(data: Int16Array, mut state: AudioState) -> AudioBuffer {
    let samples = data.length() / state.channels as u32;
    let time = state.ctx.current_time() as usize;
    if state.next_time < time {
        state.set_next_time(time);
    }
    let audio_buffer = state.ctx.create_buffer(state.channels as u32, state.bytes as u32, state.rate as f32).ok().unwrap();

    for channel in 0..state.channels {
        let mut audio_data = audio_buffer.get_channel_data(channel as u32).ok().unwrap();
        let mut offset = state.channels;
        for i in 0..samples {
            let d = Reflect::get(&data, &Number::from(offset)).ok().unwrap().as_f64().unwrap();
            audio_data[i as usize] = d as f32 / MAX_SAMPLE_VALUE as f32;
            offset = offset + state.channels;
        }
    }

    audio_buffer
}
