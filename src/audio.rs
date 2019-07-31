use web_sys::{AudioContext, AudioBuffer};

static MIN_SPLIT_SIZE: f32 = 0.02;
static MAX_SAMPLE_VALUE: usize = 32768;

// Concatenate the arrays into one array
pub fn join_packets(packets: Vec<Vec<i16>>) -> Option<Vec<i16>> {
    match packets.len() {
        0 => None,
        1 => packets.get(0)
            .map(|x| x.clone()),
        _ => Some(packets
                .iter()
                .flatten()
                .cloned()
                .collect())
    }
}

fn split_packet(data: Vec<i16>, channels: u32, rate: u32, bytes: u32) -> Vec<Vec<i16>> {
    let mut min_value = std::u32::MAX;
    let mut optimal_value = data.len();
    let samples = ((data.len() as u32 / channels) as f32).floor() as u32;
    let min_split_samples = (rate as f32 * MIN_SPLIT_SIZE).floor() as u32;
    let start = [channels * min_split_samples, channels * (samples - min_split_samples)]
        .iter()
        .max()
        .unwrap()
        .clone();

    let mut offset = start;
    while offset < data.len() as u32 {
        let mut total: u32 = 0;
        for channel in 0..channels {
            let value = data.get((offset + channel) as usize);
            total = total + value.unwrap().abs() as u32;
        }
        if (total as u32) <= min_value {
            optimal_value = (offset + channels) as usize;
            min_value = total;
        }
        offset = offset + channels;
    };

    if optimal_value == data.len() {
        return vec!(data);
    }

    let (buf1, buf2) = data.split_at(optimal_value * bytes as usize);
    vec!(buf1.to_vec(), buf2.to_vec())
}

// Convert the data into an AudioBuffer
fn to_audio_buffer(data: Vec<i16>, ctx: AudioContext, next_time: u32, channels: u32, bytes: u32, rate: u32) -> (AudioBuffer, u32) {
    let samples = data.len() as u32 / channels;
    let mut time = next_time;
    if next_time < ctx.current_time() as u32 {
        time = ctx.current_time() as u32;
    }
    let audio_buffer = ctx.create_buffer(channels, bytes, rate as f32).ok().unwrap();
    for channel in 0..channels {
        let mut audio_data = audio_buffer.get_channel_data(channel).ok().unwrap();
        let mut offset = channels;
        for i in 0..samples {
            let d = data.get(offset as usize).unwrap();
            audio_data[i as usize] = *d as f32 / MAX_SAMPLE_VALUE as f32;
            offset = offset + channels;
        }
    }
    (audio_buffer, time)
}
