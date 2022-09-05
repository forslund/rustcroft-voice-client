use std::collections::HashMap;
use std::net::TcpStream;
use std::time::Instant;

extern crate portaudio_rs as portaudio;

mod precise;
mod mycroft_stt;
use mycroft_stt::mycroft_stt;

use websocket::ClientBuilder;

extern crate rustcroft;
use rustcroft::identity::Identity;
use rustcroft::MycroftMessage;
use serde_json;

const SECONDS: f32 = 0.25;

fn rms(samples: &[i16]) -> i16
{
    let mut sum: i32 = 0;
    for &s in samples {
        sum += i32::abs(s as i32);
    }
    (sum / (samples.len() as i32)) as i16
}

fn rms_mean(samples: &[&[i16]]) -> i16 {
 1i16
}


#[derive(PartialEq)]
#[derive(Clone)]
enum RecordState {
    SpeechStarted,
    SpeechCheckForEnd,
    SpeechStopped
}

use crate::RecordState::*;


fn build_stt_message(stt_result: Vec<String>) -> websocket::OwnedMessage {
    let mut data = HashMap::<String, Vec<String>>::new();
    data.insert("utterances".into(), stt_result);
    websocket::OwnedMessage::Text(
        MycroftMessage::new("recognizer_loop:utterance")
        .with_data(serde_json::to_value(data).unwrap()).into()
    )
}

fn record(stream: portaudio::stream::Stream<i16, i16>,
          mut ws_client: websocket::sync::Client<TcpStream>)
{
    loop {
        let mut precise_engine = precise::get_runner();
        // Detect wakeword
        let mut wakeword_rms: i16 = 0;
        println!("Listening for wakeword");
        // Flush stream
        if let Ok(num_available) = stream.num_read_available() {
            let _ = stream.read(num_available);
        }

        loop {
            let input = stream.read((16384f32 * SECONDS) as u32).unwrap();
            wakeword_rms = rms(&input[..]);
            // check wakeword

            match precise_engine.get_prediction(&input[..]) {
                Ok(ww_found) => if ww_found {break},
                Err(e) => println!("{}", e),
            }
        }
        println!("Wakeword found");

        // Collect sentence
        let speech_audio = collect_sentence(&stream, wakeword_rms);
        // Send sentence audio to STT
        if let Some(sentences) = mycroft_stt(speech_audio) {
            println!("Stt says: {:?}", sentences);
            // Send sentence on messagebus
            let message = build_stt_message(sentences);
            ws_client.send_message(&message);
        } else {
            println!("Audio could not be understood :(");
        }
    }
}

fn collect_sentence(stream: &portaudio::stream::Stream<i16, i16>,
                    normal_level: i16) -> Vec::<i16> {
    let mut stt_input = Vec::<i16>::new();
    let mut state: RecordState = SpeechStarted;
    let stop_level: i16 = ((normal_level * 9) / 10).into();
    println!("Stop at {}", stop_level);
    let start_time = Instant::now();
    loop {
        let mut speech_level = 1000i16;
        let mut new_state: RecordState = state.clone();
        let input = stream.read((16000_f32 * SECONDS) as u32).unwrap();
        let current_rms = rms(&input[..]);
        // get speech
        match &state {
            SpeechStarted => {
                if start_time.elapsed().as_secs() > 1 {
                    new_state = SpeechCheckForEnd
                }
            },
            SpeechCheckForEnd => {
                if current_rms < stop_level {
                    println!("End of Speech detected");
                    new_state = SpeechStopped;
                }
            },
            _ => { panic!("UNHANDLED STATE!"); }
        }
        state = new_state;

        stt_input.append(&mut input.clone());
        if state == SpeechStopped {
            break;
        }
    }
    stt_input
}

fn main()
{
    let mut identity = Identity::load().unwrap();
    if identity.is_expired() {
        println!("Identity needs refresh");
        identity.refresh();
    } else {
        println!("Identity is valid!");
    }

    let ws_client = ClientBuilder::new("ws://localhost:8181/core")
        .unwrap()
        .connect_insecure().expect("Could not connect to Mycroft Messagebus");

    portaudio::initialize().unwrap();
    let stream: portaudio::stream::Stream<i16, i16> = 
        portaudio::stream::Stream::open_default(
            1,
            0,
            16000.0,
            portaudio::stream::FRAMES_PER_BUFFER_UNSPECIFIED,
            None).unwrap();
    stream.start().unwrap();
    record(stream, ws_client);
    portaudio::terminate().unwrap();
}
