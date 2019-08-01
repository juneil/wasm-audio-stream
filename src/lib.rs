use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{ErrorEvent, MessageEvent, WebSocket, BinaryType, AudioContext};
use js_sys::{Int16Array, ArrayBuffer, DataView};
use std::collections::VecDeque;

mod audio;

macro_rules! console_log {
    ($($t:tt)*) => (log(&format!("[wasm] {}", &format_args!($($t)*).to_string()).to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub struct AudioState {
    ws: WebSocket
}

impl AudioState {
    pub fn close(&self) {
        match self.ws.close() {
            Err(e) => console_log!("{:?}", e),
            _ => ()
        }
    }
}

#[wasm_bindgen]
pub fn start(uri: String, channels: u32, rate: u32, bytes: u32) -> AudioState {
    console_log!("Audio stream is starting...");
    let state = AudioState {
        ws: WebSocket::new_with_str(&uri, "binary").unwrap()
    };
    init_ws(&state, channels, rate, bytes);
    state
}

#[wasm_bindgen]
pub fn stop(state: AudioState) {
    console_log!("Socket closed");
    state.close();
}

/**
 * WebSocket initialisation
 * Set binary type to arraybuffer
 * Bind the callbacks
**/
fn init_ws(state: &AudioState, channels: u32, rate: u32, bytes: u32) {
    console_log!("Websocket initialisation");
    state.ws.set_binary_type(BinaryType::Arraybuffer);
    console_log!("Binary type: {:?}", state.ws.binary_type());

    let queue: VecDeque<Vec<i16>> = VecDeque::new();
    let audio_context = AudioContext::new().unwrap();
    let next_time: u32 = 0;

    let on_message_cb = on_message(queue, audio_context, next_time, channels, rate, bytes);
    let on_open_cb = on_open();
    let on_error_cb = on_error();

    state.ws.set_onmessage(Some(on_message_cb.as_ref().unchecked_ref()));
    state.ws.set_onopen(Some(on_open_cb.as_ref().unchecked_ref()));
    state.ws.set_onerror(Some(on_error_cb.as_ref().unchecked_ref()));

    on_message_cb.forget();
    on_open_cb.forget();
    on_error_cb.forget();
}

/**
 * WebSocket MESSAGE callback
**/
fn on_message(
    mut queue: VecDeque<Vec<i16>>,
    audio_context: AudioContext,
    mut next_time: u32,
    channels: u32,
    rate: u32,
    bytes: u32
) -> Closure<dyn FnMut(MessageEvent)> {
    Closure::wrap(Box::new(move |e: MessageEvent| {
        let response: ArrayBuffer = ArrayBuffer::from(e.data());
        let data = DataView::new(&response, 0, (response.byte_length() + 1) as usize);

        let mut packet: Vec<i16> = Vec::new();
        for i in 0..response.byte_length() {
            packet.push(data.get_int16(i as usize));
        }

        console_log!("RAW: {:?}", packet.len());
        // let data = Int16Array::new_with_byte_offset_and_length(&e.data(), 0, response.byte_length());
        // let data = Int16Array::from(e.data());
        // let mut packet: Vec<i16> = Vec::new();
        // data.for_each(&mut |value: i16, _, _| packet.push(value));
        // let mut values: [i16; 320] = [0; 320];
        // data.copy_to(&mut values);
        // console_log!("RAW: {:?}", packet);
        queue.push_back(packet);

        let shifted = match audio::join_packets(queue.clone()) {
            Some(data) => {
                queue = audio::split_packet(data, channels, rate, bytes);
                queue.pop_front()
            },
            None => None
        };


        match shifted {
            Some(data) => {
                let data_len = data.len() as u32;
        // console_log!("SHIFTED: {:?}", data_len);
                let packet_time = audio_context.current_time() as u32;
                if next_time < packet_time {
                    next_time = packet_time;
                }
                let source = audio_context.create_buffer_source().unwrap();
                source.connect_with_audio_node(&audio_context.destination()).unwrap();
                let (buffer, time) = audio::to_audio_buffer(data, &audio_context, next_time, channels, bytes, rate);
                next_time = time;
                source.set_buffer(Some(&buffer));
                source.start_with_when(next_time as f64).unwrap();
                next_time = next_time + (data_len / channels / rate);
            },
            None => ()
        };

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
